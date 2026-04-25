import java.util.ArrayList;
import java.util.List;

public class ThreadLeakApp {
    private static final List<Thread> WORKERS = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        System.out.println("ThreadLeakApp: spawning workers without cleanup.");
        System.out.println("Use -Xss256k and -XX:+HeapDumpOnOutOfMemoryError for easy reproduction.");

        int count = 0;
        while (true) {
            final byte[] buffer = new byte[1_048_576];
            Thread worker = new Thread(() -> {
                long heartbeat = 0;
                try {
                    while (true) {
                        heartbeat += buffer[0];
                        Thread.sleep(1_000);
                    }
                } catch (InterruptedException ignored) {
                    Thread.currentThread().interrupt();
                }
            }, "leaky-worker-" + count);
            worker.setDaemon(true);
            worker.start();
            WORKERS.add(worker);

            if (count % 25 == 0) {
                long approxMb = WORKERS.size();
                System.out.printf("Workers: %d (~%d MB in retained buffers)%n", WORKERS.size(), approxMb);
            }

            count++;
            Thread.sleep(100);
        }
    }
}