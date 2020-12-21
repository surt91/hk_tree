#!/usr/bin/env python3

def parse_file_generator(filename):
    with open(filename) as f:
        for line in f:
            if line.startswith("# sweeps:"):
                speed = float(line[9:])
            elif line.startswith("#"):
                positions = list(map(float, line[2:].split()))
            else:
                sizes = list(map(float, line.split()))

                # for every dataset (consisting of 3 lines) return one result
                yield positions, sizes, speed


# if started as a program, calculate <S>(eps) and show it
if __name__ == "__main__":
    import os
    # if matplotlib is installed, visualize the results
    try:
        import matplotlib.pyplot as plt
    except:
        vis = False
    else:
        vis = True
        x = []
        y = []

    # do a fairly small simulation, such that it will finish fast on a laptop
    N = 1024
    samples = 100
    seed = 0

    os.makedirs("data", exist_ok=True)
    files = []
    cmds = []
    # first simulate some low resolution data
    for eps in range(0, 51):
        seed +=1
        eps /= 100.
        outname = "data/out_n{}_e{:.2f}.dat".format(N, eps)
        if not os.path.exists("target/release/hk"):
            os.system("cargo build --release")
        cmd = "target/release/hk -l {} -u {} -n {} --samples {} --seed {} -o {}".format(eps, eps, N, samples, seed, outname)
        files.append(outname)

        if not os.path.exists(outname):
            cmds.append(cmd)

    print("# starting simulation of missing result files", flush=True)
    from multiprocessing import Pool
    def progress(x):
        os.system(x)
        print(".", end="", flush=True)

    with Pool() as p:
        p.map(progress, cmds)
        print()

    # then evaluate them: here by calculating the mean size of the largest cluster
    for file in files:
        # extract confidence value from filename
        eps_string = file.split("_e")[1].split(".dat")[0]
        confidence = float(eps_string)

        avg_S = 0
        samples = 0
        for dataset in parse_file_generator(file):
            sizes = dataset[1]
            avg_S += max(sizes) / N
            samples += 1
        avg_S /= samples

        if vis:
            x.append(confidence)
            y.append(avg_S)
        else:
            # if we can not visualize, print the results to stdout
            print(confidence, avg_S)

    if vis:
        plt.xlabel("confidence")
        plt.ylabel("<S>")
        plt.plot(x, y)
        plt.show()

