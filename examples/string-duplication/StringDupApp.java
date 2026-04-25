import java.util.ArrayList;
import java.util.List;

public class StringDupApp {
    private static final String[] CITIES = {
        "New York", "London", "Tokyo", "Paris", "Sydney", "Berlin"
    };
    private static final String[] STATUSES = {
        "ACTIVE", "PENDING", "ACTIVE", "ACTIVE", "PENDING", "ACTIVE"
    };

    public static void main(String[] args) {
        List<String> cityValues = new ArrayList<>();
        List<String> statusValues = new ArrayList<>();
        List<String> csvRows = new ArrayList<>();

        System.out.println("StringDupApp: duplicating strings from CSV-like input.");
        System.out.println("Use -XX:+HeapDumpOnOutOfMemoryError to capture the heap automatically.");

        long rows = 0;
        while (true) {
            for (int batch = 0; batch < 5_000; batch++) {
                String cityToken = CITIES[(int) ((rows + batch) % CITIES.length)];
                String statusToken = STATUSES[(int) ((rows + batch) % STATUSES.length)];

                // Force fresh String instances instead of reusing the same logical values.
                cityValues.add(new String(cityToken.toCharArray()));
                statusValues.add(new String(statusToken.toCharArray()));
                csvRows.add(new String(("city=" + cityToken + ",status=" + statusToken).toCharArray()));
            }
            rows += 5_000;

            if (rows % 100_000 == 0) {
                System.out.printf(
                    "Rows parsed: %,d (%d duplicate fields, %d duplicate rows)%n",
                    rows,
                    cityValues.size() + statusValues.size(),
                    csvRows.size()
                );
            }
        }
    }
}