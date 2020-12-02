C benchmark:


    ---- Start execution at time Wed Dec  2 16:05:22 2020
    ---- plus 316029230 nanoseconds.
    PingPongBenchmark
    numIterations: 20, count: 1000000
    System information
    O/S Name: Linux
    Iteration: 1	 Duration: 119.559 msec
    Iteration: 2	 Duration: 115.035 msec
    Iteration: 3	 Duration: 115.573 msec
    Iteration: 4	 Duration: 114.793 msec
    Iteration: 5	 Duration: 115.780 msec
    Iteration: 6	 Duration: 115.143 msec
    Iteration: 7	 Duration: 114.927 msec
    Iteration: 8	 Duration: 115.304 msec
    Iteration: 9	 Duration: 114.432 msec
    Iteration: 10	 Duration: 114.696 msec
    Iteration: 11	 Duration: 115.577 msec
    Iteration: 12	 Duration: 114.367 msec
    Iteration: 13	 Duration: 115.010 msec
    Iteration: 14	 Duration: 114.137 msec
    Iteration: 15	 Duration: 114.792 msec
    Iteration: 16	 Duration: 115.816 msec
    Iteration: 17	 Duration: 116.285 msec
    Iteration: 18	 Duration: 115.195 msec
    Iteration: 19	 Duration: 114.745 msec
    Iteration: 20	 Duration: 115.759 msec
    Execution - Summary:
    Best Time:	 115.195 msec
    Worst Time:	 115.816 msec
    Median Time:	 114.969 msec
    ---- Elapsed logical time (in nsec): 0
    ---- Elapsed physical time (in nsec): 2,307,282,069




Rust:



    PingPongBenchmark
    numIterations: 20, count: 1000000
    Iteration: 1	 Duration: 1068 ms
    Iteration: 2	 Duration: 1064 ms
    Iteration: 3	 Duration: 1061 ms
    Iteration: 4	 Duration: 1063 ms
    Iteration: 5	 Duration: 1059 ms
    Iteration: 6	 Duration: 1062 ms
    Iteration: 7	 Duration: 1060 ms
    Iteration: 8	 Duration: 1060 ms
    Iteration: 9	 Duration: 1059 ms
    Iteration: 10	 Duration: 1066 ms
    Iteration: 11	 Duration: 1062 ms
    Iteration: 12	 Duration: 1085 ms
    Iteration: 13	 Duration: 1064 ms
    Iteration: 14	 Duration: 1063 ms
    Iteration: 15	 Duration: 1062 ms
    Iteration: 16	 Duration: 1065 ms
    Iteration: 17	 Duration: 1061 ms
    Iteration: 18	 Duration: 1063 ms
    Iteration: 19	 Duration: 1060 ms
    Iteration: 20	 Duration: 1061 ms

    Exec summary
    Best time:	1059 ms
    Worst time:	1085 ms
    Median time:	1062 ms
