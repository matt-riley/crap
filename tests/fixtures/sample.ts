function trivial(): number { return 1 }                            // cc=1

function binaryOps(a: boolean, b: boolean): boolean { return a && b || a }  // cc=3

function nullish(a: string | null, b: string): string {            // cc=2
  return a ?? b;
}

function outer(a: boolean): number {                               // cc=2 (nested excluded)
  const inner = (b: boolean) => (b ? 1 : 2);                       // cc=2
  return a ? inner(a) : 0;
}

class Service {
  run(x: number): number {                                        // cc=3
    for (const i of [1, 2]) { if (i > x) return i }
    return 0
  }
  get value(): number { return 1 }                                 // cc=1
}

function guard(x: unknown): number {                               // cc=2
  try {
    return Number(x)
  } catch (error) {
    throw error
  } finally {
    return 0
  }
}

const assigned = { handler: (x: number) => (x > 0 ? x : -x) };      // cc=2
