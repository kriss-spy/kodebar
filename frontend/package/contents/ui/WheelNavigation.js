.pragma library

function consumeDelta(delta, remainder, stepSize) {
    if (typeof delta !== "number" || !isFinite(delta)
            || typeof remainder !== "number" || !isFinite(remainder)
            || typeof stepSize !== "number" || !isFinite(stepSize) || stepSize <= 0) {
        return { steps: 0, remainder: 0 };
    }

    const total = remainder + delta;
    const steps = total < 0
        ? Math.ceil(total / stepSize)
        : Math.floor(total / stepSize);
    return {
        steps: steps,
        remainder: total - steps * stepSize,
    };
}
