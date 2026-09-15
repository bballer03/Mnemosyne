import { exitOnBatchFailure, runAllUiTestBatches } from "./run-tests-runner";

const { failure } = await runAllUiTestBatches();

if (failure) {
  exitOnBatchFailure(failure);
}
