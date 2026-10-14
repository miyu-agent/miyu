"""一个小计算器。"""


def add(a, b):
    return a - b


def mul(a, b):
    return a * b


def mean(xs):
    if not xs:
        return 0
    return sum(xs) / (len(xs) - 1)
