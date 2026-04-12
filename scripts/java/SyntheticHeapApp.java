import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.concurrent.TimeUnit;

public final class SyntheticHeapApp {
    private static final int ONE_MIB = 1024 * 1024;
    private static final int DUPLICATE_POOL_SIZE = 256;
    private static final int DUPLICATE_PADDING = 64;
    private static final int UNIQUE_LABEL_PADDING = 96;
    private static final int NODE_LABEL_PADDING = 48;
    private static final int CLUSTER_PAYLOAD_BYTES = 8 * 1024;
    private static final int CLUSTER_TEXT_CHARS = 4 * 1024;
    private static final int CLUSTER_COUNTERS = 2 * 1024;

    private SyntheticHeapApp() {
    }

    public static void main(String[] args) throws Exception {
        int targetMb = parseTargetMb(args);
        HeapLayout layout = allocateLayout(targetMb);

        System.out.printf("READY %d %d%n", ProcessHandle.current().pid(), layout.roots.size());
        System.out.flush();

        while (true) {
            if (layout.roots.isEmpty()) {
                throw new IllegalStateException("synthetic roots unexpectedly empty");
            }
            TimeUnit.MINUTES.sleep(1);
        }
    }

    private static int parseTargetMb(String[] args) {
        if (args.length != 1) {
            throw new IllegalArgumentException("expected target size in MiB");
        }

        int targetMb = Integer.parseInt(args[0]);
        if (targetMb <= 0) {
            throw new IllegalArgumentException("target size must be positive");
        }
        return targetMb;
    }

    private static HeapLayout allocateLayout(int targetMb) {
        long targetBytes = (long) targetMb * ONE_MIB;
        long allocatedBytes = 0;
        int seed = 0;

        ArrayList<Object> roots = new ArrayList<>();
        ArrayList<Cluster> clusters = new ArrayList<>();
        ArrayList<String> duplicatePool = buildDuplicatePool();
        roots.add(duplicatePool);

        while (allocatedBytes < targetBytes) {
            Cluster cluster = buildCluster(seed++, clusters, duplicatePool, targetMb);
            clusters.add(cluster);
            roots.add(cluster);
            allocatedBytes += cluster.approxBytes;
        }

        roots.add(buildCrossClusterIndex(clusters));
        roots.add(buildClusterMap(clusters));

        if (clusters.isEmpty()) {
            throw new IllegalStateException("synthetic cluster allocation produced no clusters");
        }

        return new HeapLayout(roots, clusters);
    }

    private static ArrayList<String> buildDuplicatePool() {
        ArrayList<String> duplicates = new ArrayList<>(DUPLICATE_POOL_SIZE);
        String padding = "y".repeat(DUPLICATE_PADDING);
        for (int i = 0; i < DUPLICATE_POOL_SIZE; i++) {
            String value = "tenant-" + (i % 32) + "-session-" + (i % 16) + "-" + padding;
            duplicates.add(new String(value.toCharArray()));
        }
        return duplicates;
    }

    private static Object[] buildCrossClusterIndex(ArrayList<Cluster> clusters) {
        Object[] index = new Object[Math.max(64, clusters.size() * 4)];
        for (int i = 0; i < index.length; i++) {
            Cluster cluster = clusters.get(i % clusters.size());
            index[i] = cluster.nodes[(i * 7) % cluster.nodes.length];
        }
        return index;
    }

    private static HashMap<String, Cluster> buildClusterMap(ArrayList<Cluster> clusters) {
        HashMap<String, Cluster> clusterMap = new HashMap<>(Math.max(16, clusters.size() * 2));
        for (int i = 0; i < clusters.size(); i++) {
            clusterMap.put("cluster-" + i, clusters.get(i));
        }
        return clusterMap;
    }

    private static Cluster buildCluster(
        int seed,
        ArrayList<Cluster> existingClusters,
        ArrayList<String> duplicatePool,
        int targetMb
    ) {
        int nodeCount = nodeCountForTarget(targetMb);
        Node[] nodes = new Node[nodeCount];
        ArrayList<Node> nodeList = new ArrayList<>(nodeCount);
        HashMap<String, Node> nodeMap = new HashMap<>(nodeCount * 2);
        ArrayList<String> duplicateRefs = new ArrayList<>(nodeCount);
        ArrayList<String> uniqueLabels = new ArrayList<>(nodeCount);
        Object[] wrappers = new Object[Math.max(256, nodeCount / 4)];
        byte[] payload = new byte[CLUSTER_PAYLOAD_BYTES];
        char[] text = new char[CLUSTER_TEXT_CHARS];
        int[] counters = new int[CLUSTER_COUNTERS];
        String uniquePadding = "u".repeat(UNIQUE_LABEL_PADDING);
        String nodePadding = "n".repeat(NODE_LABEL_PADDING);

        for (int i = 0; i < payload.length; i++) {
            payload[i] = (byte) (seed + i);
        }
        for (int i = 0; i < text.length; i++) {
            text[i] = (char) ('a' + ((seed + i) % 26));
        }
        for (int i = 0; i < counters.length; i++) {
            counters[i] = seed * 31 + i;
        }

        for (int i = 0; i < nodeCount; i++) {
            String label = "cluster-" + seed + "-node-" + i + "-" + nodePadding;
            String unique = "cluster-" + seed + "-unique-" + i + "-" + uniquePadding;
            String duplicate = duplicatePool.get((seed * 17 + i) % duplicatePool.size());
            MiniPayload miniPayload = new MiniPayload(i, duplicate, unique, counters[i % counters.length]);
            Node node = new Node(new String(label.toCharArray()), i, miniPayload);
            nodes[i] = node;
            nodeList.add(node);
            nodeMap.put(label, node);
            duplicateRefs.add(duplicate);
            uniqueLabels.add(new String(unique.toCharArray()));
        }

        for (int i = 0; i < nodeCount; i++) {
            Node node = nodes[i];
            node.next = nodes[(i + 1) % nodeCount];
            node.back = nodes[(i + nodeCount - 1) % nodeCount];
            node.jump = nodes[(i + 31) % nodeCount];
            node.alias = nodes[(i * 13 + 7) % nodeCount];
            node.label = uniqueLabels.get(i);
            node.sharedLabel = duplicateRefs.get(i);
            node.bucket = duplicateRefs.subList(Math.max(0, i - 2), Math.min(duplicateRefs.size(), i + 1));
            wrappers[i % wrappers.length] = node;
        }

        for (int i = 0; i < wrappers.length; i++) {
            wrappers[i] = i % 3 == 0 ? nodes[(i * 5) % nodeCount] : duplicateRefs.get(i % duplicateRefs.size());
        }

        if (!existingClusters.isEmpty()) {
            Cluster previous = existingClusters.get(existingClusters.size() - 1);
            for (int i = 0; i < Math.min(128, nodeCount); i++) {
                nodes[i].external = previous.nodes[(seed + i) % previous.nodes.length];
            }
        }

        return new Cluster(
            seed,
            nodes,
            nodeList,
            nodeMap,
            duplicateRefs,
            uniqueLabels,
            wrappers,
            payload,
            text,
            counters,
            estimateClusterBytes(nodeCount)
        );
    }

    private static int nodeCountForTarget(int targetMb) {
        if (targetMb <= 32) {
            return 8000;
        }
        if (targetMb <= 128) {
            return 12000;
        }
        if (targetMb <= 512) {
            return 16000;
        }
        return 22000;
    }

    private static long estimateClusterBytes(int nodeCount) {
        long nodeBytes = (long) nodeCount * 256L;
        long stringBytes = (long) nodeCount * 192L;
        long collectionBytes = (long) nodeCount * 96L;
        long fixedBytes = CLUSTER_PAYLOAD_BYTES + ((long) CLUSTER_TEXT_CHARS * Character.BYTES)
            + ((long) CLUSTER_COUNTERS * Integer.BYTES);
        return nodeBytes + stringBytes + collectionBytes + fixedBytes;
    }

    private static final class HeapLayout {
        private final ArrayList<Object> roots;
        @SuppressWarnings("unused")
        private final ArrayList<Cluster> clusters;

        private HeapLayout(ArrayList<Object> roots, ArrayList<Cluster> clusters) {
            this.roots = roots;
            this.clusters = clusters;
        }
    }

    private static final class Cluster {
        @SuppressWarnings("unused")
        private final int id;
        private final Node[] nodes;
        @SuppressWarnings("unused")
        private final ArrayList<Node> nodeList;
        @SuppressWarnings("unused")
        private final HashMap<String, Node> nodeMap;
        @SuppressWarnings("unused")
        private final ArrayList<String> duplicateRefs;
        @SuppressWarnings("unused")
        private final ArrayList<String> uniqueLabels;
        @SuppressWarnings("unused")
        private final Object[] wrappers;
        @SuppressWarnings("unused")
        private final byte[] payload;
        @SuppressWarnings("unused")
        private final char[] text;
        @SuppressWarnings("unused")
        private final int[] counters;
        private final long approxBytes;

        private Cluster(
            int id,
            Node[] nodes,
            ArrayList<Node> nodeList,
            HashMap<String, Node> nodeMap,
            ArrayList<String> duplicateRefs,
            ArrayList<String> uniqueLabels,
            Object[] wrappers,
            byte[] payload,
            char[] text,
            int[] counters,
            long approxBytes
        ) {
            this.id = id;
            this.nodes = nodes;
            this.nodeList = nodeList;
            this.nodeMap = nodeMap;
            this.duplicateRefs = duplicateRefs;
            this.uniqueLabels = uniqueLabels;
            this.wrappers = wrappers;
            this.payload = payload;
            this.text = text;
            this.counters = counters;
            this.approxBytes = approxBytes;
        }
    }

    private static final class Node {
        private Node next;
        private Node back;
        private Node jump;
        private Node alias;
        private Node external;
        private Object payload;
        private String label;
        private String sharedLabel;
        private List<String> bucket;
        private final int weight;

        private Node(String label, int weight, Object payload) {
            this.label = label;
            this.weight = weight;
            this.payload = payload;
        }
    }

    private static final class MiniPayload {
        private final int id;
        @SuppressWarnings("unused")
        private final String duplicate;
        @SuppressWarnings("unused")
        private final String unique;
        @SuppressWarnings("unused")
        private final int score;

        private MiniPayload(int id, String duplicate, String unique, int score) {
            this.id = id;
            this.duplicate = duplicate;
            this.unique = unique;
            this.score = score;
        }
    }
}
