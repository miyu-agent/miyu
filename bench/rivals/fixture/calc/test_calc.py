from calc import add, mean, mul


def main():
    failures = 0
    cases = [
        ("add", add(2, 3), 5),
        ("mul", mul(4, 5), 20),
        ("mean", mean([1, 2, 3, 4]), 2.5),
        ("mean empty", mean([]), 0),
    ]
    for name, got, want in cases:
        if got != want:
            print(f"FAIL {name}: got {got}, want {want}")
            failures += 1
    print("ALL PASS" if failures == 0 else f"{failures} FAILED")
    raise SystemExit(1 if failures else 0)


main()
