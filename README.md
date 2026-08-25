    # PaTeX (Pentaract-Pexil TeX)

    > **The ASCII Equivalent of LaTeX for 5D Geometric Topology & Discrete Spatial Manifolds**

    [![Specification](https://img.shields.io/badge/Spec-v2.1.0-blue.svg)](06_PATEX_5D_GEOMETRIC_TYPESETTING.md)
    [![Manifold](https://img.shields.io/badge/Manifold-M%5E5%20%E2%89%85%20S%5E4%20%C3%97%20Z%5E5-brightgreen.svg)](#mathematical-overview)
    [![Projection](https://img.shields.io/badge/Axonometry-2%3A1%20Dimetric-orange.svg)](#true-21-dimetric-axonometry)
    [![Accessibility](https://img.shields.io/badge/Accessibility-Screen--Reader%20Linearized-purple.svg)](#accessibility--screen-reader-linearization)

    ---

    ## Overview

    **PaTeX** (Pentaract-Pexil $\TeX$) is an algebraic typesetting language and deterministic rasterization grammar designed to represent high-dimensional discrete spatial structures within bounded
  monospace terminal surfaces.

    While traditional document preparation systems ($\TeX$, $\LaTeX$) excel at formatting 1D linear text streams and mathematical equations, they lack intrinsic spatial dimensionality and discrete
  topological connectivity. PaTeX establishes a formal, lossless bijection between a **5-dimensional manifold ($M^5 \cong S^4 \times \mathbb{Z}^5$)** and **71-column ASCII/Unicode monospace viewports**:

    $$\mathcal{P}: M^5 \longleftrightarrow \Sigma^{71 \times 48}$$

    ---

    ## Key Features

    - **5-Dimensional Manifold ($M^5$):** Unifies spatial coordinates $(x, y, z)$, temporal causality phases $(t)$, and semantic functional lanes $(s)$ in a single balanced ternary tensor $\mathbf{p} \in
  \{-1, 0, +1\}^5$.
    - **Continuous $S^4$ Hypersphere Apertures:** Models continuous camera perspectives and hyper-dimensional mood vectors via 4-angle hyperspherical coordinates $(\theta_1, \theta_2, \theta_3, \phi)$
  with exact 5D unit vector embedding.
    - **44-Glyph Unicode Box-Drawing Algebra:** Evaluates 4-connected spatial neighbor tensors $(N, E, S, W)$ directly into single, double, and mixed junctions via a closed-form radix-3 connectivity key:
      $$\text{Key}(\mathbf{N}) = N + 3E + 9S + 27W$$
    - **True 2:1 Dimetric Axonometry:** Derives an exact integer 2:1 slope ($\text{RUN} = 2, \text{RISE} = 1$) directly from the ternary lattice, avoiding floating-point sine/cosine evaluations and
  trigonometric drift.
    - **Screen-Reader Linearization:** Built-in structural accessibility that compresses consecutive material runs and applies the *void silence invariant* for fluid auditory terminal telemetry.

    ---

    ## Syntax Example (`.ptex`)

    ```patex
    \begin{patex}
    \chamber{VAULT_SANCTUM}
    \stratum{-1}
    \phase{Present}
    \density{0.8400}

    \topology{
      ╔═════════════════════════════════════════════════════════════════════╗
      ║ [STRATUM -1: VAULT SANCTUM]                   [PHASE: T0 PRESENT]   ║
      ╠═════════════════════════════════════════════════════════════════════╣
      ║  ┌─[NORTH PORTAL]─┐                                                 ║
      ║  │ . . . . . . . .│ . . . . . . . . . . . . . . . . . . . . . . .   ║
      ║  │ . . ┌─────┐ . .│ . . . . . . . . . . . . ┌─────┐ .               ║
      ║  │ . . │ (1) │ . .│ . . . [ALTAR] . . . . . │ (2) │ .               ║
      ║  │ . . └─────┘ . .│ . . .  0x8F2C . . . . . └─────┘ .               ║
      ║  │ . . . . . . . .│ . . . . . . . . . . . . . . . . . . . . . . .   ║
      ║  └─┬─────────────┬┘                                                 ║
      ║    │             │                                                 ║
      ║    └──────[SIGHT]┘                                                 ║
      ╚═════════════════════════════════════════════════════════════════════╝
    }
    \end{patex}
  ──────
  ## Mathematical Overview

  ### 1. Horner Radix-3 Lattice Packing

  The 243 discrete interior states of a 5D balanced ternary cell (t₀,t₁,t₂,t₃,t₄) ∈ { - 1,0, + 1}⁵ are packed into a single linear byte index I ∈ [0,242] via Horner's rule:

    I = (t₀ + 1) + 3 Big ((t₁ + 1) + 3 Big ((t₂ + 1) + 3 Big ((t₃ + 1) + 3(t₄ + 1) Big) Big) Big)

  ### 2. Disjoint Lane Partitioning

  The fifth trit t₄ partitions the 243 states into three mutually exclusive 81-state domains:

  1. Material Lane (t₄ = -1, I ∈ [0,80]): Occlusion fields, mist, haze, and structural boundaries.
  2. Topology Lane (t₄ = 0, I ∈ [81,161]): 4-connected single and double box-drawing geometry.
  3. Mark Lane (t₄ = +1, I ∈ [162,242]): Semantic altars, entity anchors, floor symbols, and flux bands.

  ### 3. Continuous S⁴ Hypersphere Embedding

  For continuous orientations and light transport, points on the 4-sphere S⁴ ⊂ ℝ⁵ project into 5D unit space:

    x₁ = cos θ₁

    x₂ = sin θ₁ cos θ₂

    x₃ = sin θ₁ sin θ₂ cos θ₃

    x₄ = sin θ₁ sin θ₂ sin θ₃ cos φ

    x₅ = sin θ₁ sin θ₂ sin θ₃ sin φ

     5
     ∑  x²ᵢ = 1
    i=1
  ──────
  ## Documentation

  • Full Conceptual Whitepaper /06_PATEX_5D_GEOMETRIC_TYPESETTING.md: Complete mathematical derivations, the 44-glyph box-drawing table, multi-view section cuts, and screen-reader linearization algorithms.
  ──────
  ## Citation

  If you use PaTeX in your research or software, please cite:

    @article{patex2026,
      title   = {PaTeX: The ASCII Equivalent of 5D LaTeX for Hyper-Dimensional Topology and Discrete Manifolds},
      author  = {The 13Forge Architecture Group},
      year    = {2026},
      journal = {Technical Whitepaper Series},
      volume  = {6},
      url     = {https://github.com/your-username/patex}
    }
  ──────
  ## License

  MIT or Apache-2.0.
