# Mathematical Notes

A stress test for Postilla's Typst rendering. Every formula below lives in a
`typst` code block, so the preview, the PDF export and the HTML export all
draw the same thing.

## 1. Analysis

The Gaussian integral and its generalisation to a quadratic form in
`n` dimensions:

```typst
$ integral_(-oo)^(oo) e^(-x^2) dif x = sqrt(pi),
  quad
  integral_(bb(R)^n) exp(-1/2 bold(x)^top bold(A) bold(x) + bold(b)^top bold(x)) dif^n x
  = sqrt((2 pi)^n / det bold(A)) exp(1/2 bold(b)^top bold(A)^(-1) bold(b)) $
```

The Fourier transform pair, with the convention used in signal processing:

```typst
$ hat(f)(xi) = integral_(-oo)^(oo) f(x) e^(-2 pi i x xi) dif x,
  quad
  f(x) = integral_(-oo)^(oo) hat(f)(xi) e^(2 pi i x xi) dif xi $
```

Taylor's theorem with the Lagrange form of the remainder:

```typst
$ f(x) = sum_(k=0)^(n) (f^((k))(a)) / (k!) (x - a)^k
  + (f^((n+1))(xi)) / ((n+1)!) (x - a)^(n+1),
  quad xi in (a, x) $
```

A limit that defines the exponential, and an epsilon-delta statement:

```typst
$ lim_(n -> oo) (1 + x/n)^n = e^x,
  quad
  forall epsilon > 0, exists delta > 0 : abs(x - a) < delta => abs(f(x) - f(a)) < epsilon $
```

## 2. Complex analysis and number theory

Cauchy's integral formula and the residue theorem:

```typst
$ f(a) = 1 / (2 pi i) integral.cont_gamma f(z) / (z - a) dif z,
  quad
  integral.cont_gamma f(z) dif z = 2 pi i sum_(k=1)^(n) "Res"(f, a_k) $
```

The Riemann zeta function as a Dirichlet series and as an Euler product, plus
its functional equation:

```typst
$ zeta(s) = sum_(n=1)^(oo) 1 / n^s = product_(p "prime") 1 / (1 - p^(-s)),
  quad "Re"(s) > 1 $

$ zeta(s) = 2^s pi^(s-1) sin((pi s) / 2) Gamma(1 - s) zeta(1 - s) $
```

Euler's identity, stated once because every note needs it:

```typst
$ e^(i pi) + 1 = 0 $
```

## 3. Linear algebra

Eigendecomposition and the singular value decomposition of a real matrix:

```typst
$ bold(A) = bold(Q) bold(Lambda) bold(Q)^(-1),
  quad
  bold(Lambda) = mat(
    lambda_1, 0, dots.c, 0;
    0, lambda_2, dots.c, 0;
    dots.v, dots.v, dots.down, dots.v;
    0, 0, dots.c, lambda_n
  ),
  quad
  bold(M) = bold(U) bold(Sigma) bold(V)^top $
```

The determinant of a 3 by 3 matrix, expanded along the first row:

```typst
$ det mat(delim: "|", a, b, c; d, e, f; g, h, i)
  = a (e i - f h) - b (d i - f g) + c (d h - e g) $
```

A block matrix inverse via the Schur complement:

```typst
$ mat(bold(A), bold(B); bold(C), bold(D))^(-1)
  = mat(
    bold(A)^(-1) + bold(A)^(-1) bold(B) bold(S)^(-1) bold(C) bold(A)^(-1),
      - bold(A)^(-1) bold(B) bold(S)^(-1);
    - bold(S)^(-1) bold(C) bold(A)^(-1),
      bold(S)^(-1)
  ),
  quad bold(S) = bold(D) - bold(C) bold(A)^(-1) bold(B) $
```

## 4. Physics

Maxwell's equations in differential form:

```typst
$ nabla dot bold(E) &= rho / epsilon_0 \
  nabla dot bold(B) &= 0 \
  nabla times bold(E) &= - (partial bold(B)) / (partial t) \
  nabla times bold(B) &= mu_0 bold(J) + mu_0 epsilon_0 (partial bold(E)) / (partial t) $
```

The time dependent Schrodinger equation and the Hamiltonian of a particle in
a potential:

```typst
$ i planck partial / (partial t) Psi(bold(r), t)
  = hat(H) Psi(bold(r), t),
  quad
  hat(H) = - planck^2 / (2m) nabla^2 + V(bold(r)) $
```

Navier-Stokes for an incompressible Newtonian fluid:

```typst
$ rho ((partial bold(u)) / (partial t) + (bold(u) dot nabla) bold(u))
  = - nabla p + mu nabla^2 bold(u) + bold(f),
  quad nabla dot bold(u) = 0 $
```

The Euler-Lagrange equation and the action it extremises:

```typst
$ S[q] = integral_(t_1)^(t_2) cal(L)(q, dot(q), t) dif t,
  quad
  dif / (dif t) ((partial cal(L)) / (partial dot(q)_i)) - (partial cal(L)) / (partial q_i) = 0 $
```

Einstein's field equations, with the cosmological constant:

```typst
$ R_(mu nu) - 1/2 R g_(mu nu) + Lambda g_(mu nu) = (8 pi G) / c^4 T_(mu nu) $
```

## 5. Probability and statistics

Bayes' theorem, the normal density and the central limit theorem:

```typst
$ P(A | B) = (P(B | A) P(A)) / P(B),
  quad
  phi(x; mu, sigma^2) = 1 / sqrt(2 pi sigma^2) exp(- (x - mu)^2 / (2 sigma^2)) $

$ sqrt(n) (overline(X)_n - mu) -->^d cal(N)(0, sigma^2) quad "as" n -> oo $
```

A piecewise definition and a binomial expansion:

```typst
$ f(x) = cases(
    x^2 sin(1/x) & "if" x != 0,
    0 & "if" x = 0
  ),
  quad
  (x + y)^n = sum_(k=0)^(n) binom(n, k) x^(n-k) y^k $
```

## 6. A derivation with alignment

Completing the square to solve the general quadratic:

```typst
$ a x^2 + b x + c &= 0 \
  x^2 + b/a x &= - c/a \
  (x + b/(2a))^2 &= b^2/(4a^2) - c/a = (b^2 - 4 a c) / (4 a^2) \
  x &= (- b plus.minus sqrt(b^2 - 4 a c)) / (2a) $
```

## 7. Inline notation in prose

Typst blocks are display math only, so inline symbols stay as plain text or
code: the golden ratio `phi = (1 + sqrt(5)) / 2`, the set of reals `RR`, and
the Landau notation `O(n log n)`. Everything that needs real typesetting goes
in a block above.

---

Rendered offline by the Typst engine bundled with Postilla. No network, no
external service.
