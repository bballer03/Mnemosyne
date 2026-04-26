import java.util.HashMap;
import java.util.Map;

public class CacheLeakApp {
    private static final Map<String, byte[]> CACHE = new HashMap<>();

    public static void main(String[] args) {
        System.out.println("CacheLeakApp: growing an unbounded HashMap cache.");
        System.out.println("Use -XX:+HeapDumpOnOutOfMemoryError to capture the leak automatically.");

        int count = 0;
        while (true) {
            // Roughly 10 KiB per entry, retained forever by the static cache.
            String key = "cache-key-" + count;
            byte[] value = new byte[10_240];
            CACHE.put(key, value);

            if (count % 1_000 == 0) {
                long approxMb = (CACHE.size() * 10L) / 1024;
                System.out.printf("Cache size: %d entries (~%d MB retained)%n", CACHE.size(), approxMb);
            }
            count++;
        }
    }
}