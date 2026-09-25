# Flagship Theory of Intelligence

> 主题册一·智能理论旗舰 · TwinsEarth · 2026-09-24 · Papers released under CC BY 4.0

---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Structure Is the Intelligence

**Working Title (EN):** *Structure Is the Intelligence: Spatial Organization as the Substrate-Invariant Core of Intelligent Systems*

> Flagship synthesis paper P0 of the whole volume · UDOS Reasoning Engine v7.5.0 · 2026-09-19
> This paper is the master outline of the 12 topical papers (P1–P12). It adds no independent experiment; rather, it converges the measured evidence scattered across the topical papers and the established knowledge spanning the four natural-science chapters onto one falsifiable central claim.
> Evidence discipline: UDOS figures all trace back to the raw JSON in reports7/, marked verified / cpu-proto / cpu-proxy / unverified; single-seed pilots are written as "pilot point estimates." Natural-science classics were verified one by one on 2026-09-19, and those that could not be verified are marked [CITATION NEEDED].

---

## Abstract

We advance a restrained, falsifiable central claim: **given fixed components (materials, weights) and a fixed energy budget, the intelligent behavior a system exhibits is primarily determined by its spatial relational structure; structure is the substrate-invariant core across chemical bonds, carbon chains, proteins, neurons, and artificial machines or agent collectives, while material and substrate are replaceable implementations.**

We define "structure" strictly as a triple: *connectivity topology* (who connects to whom), *geometry/spatial organization* (metric and orientation of relations), and *modular boundaries* (hierarchy and encapsulation), and distinguish it from "parameters" (weights, material composition) and "dynamics" (energy, learning rules). Around this definition we state four falsifiable predictions, each with an explicit refutation condition. Evidence comes from two roads: a natural-history road (isomerism, protein denaturation, connective lesions) and an engineering road measured on the UDOS engine (star-vs-layered topology, multi-view pose structure, hybrid embodied control, and the unidentifiability of dynamics when structural information is insufficient). We do not claim structure is sufficient; Chapter 6 fully voices the objections. Structure is not everything, but it is the fourth axis—structure axis—systematically under-counted by the prevailing parameter–data–compute scaling narrative.

**Keywords**: structure; connectivity topology; spatial organization; modular boundaries; substrate invariance; falsifiable prediction; fourth scaling axis; UDOS; multi-agent; embodied intelligence

---

## 1 Introduction

### 1.1 A fact repeatedly seen yet rarely named

Across different scales, "intelligence" repeatedly points to the same non-material thing. A protein's catalytic power is determined not by what its amino-acid atoms are but by the three-dimensional shape those atoms fold into; DNA can replicate itself not because it is made of carbon, hydrogen, oxygen, nitrogen, and phosphorus (countless molecules are made of the same elements) but because it is a double helix wound by complementary base pairing; the human brain can remember and plan not in the firing of a single neuron but in the connectivity among its eighty billion neurons. Placing these facts together, a simple question surfaces: **if the material is changed while the relational structure is preserved, does the function remain?**

This paper elevates that question into a researchable, falsifiable proposition and argues: separating "structure" from "materials/parameters" and from "dynamics/rules" is an overlooked key to understanding intelligent systems, whether biological or artificial.

Boundaries must be drawn first in the introduction. This paper does not use undefined big words like "universe," "ultimate," or "first principles." The "structure" this paper speaks of is a triple that can be operationalized, measured, and broken in Chapter 2; the "intelligent behavior" this paper speaks of refers to the coordination, prediction, generalization, and control a system exhibits on a given task, not any metaphysical "consciousness." The argumentation strategy is restrained: the introduction and conclusion only state the theme, while the full weight of the body is pressed onto evidence and formalization.

### 1.2 Why raise this now

Over the past decade, the success of large models has been almost monopolized by one narrative: treating parameter count, data volume, and compute as three coordinate axes, with performance falling along a power law (Kaplan et al., 2020; Hoffmann et al., 2022, i.e. Chinchilla). This narrative is extremely successful, but it has a systematic blind spot: it assumes "what the network looks like" is fixed—all use the Transformer, all stack layers, all stack width, all connect by some established attention pattern—and then only adds parameters along this fixed structural track. Once "structure" is also made a designable variable, a hidden dimension emerges:

- With the same parameters, switching to sparse routing (MoE / Switch, Fedus et al., 2021) multiplies parameter capacity several-fold without increasing activated compute;
- With the same number of agents, a star center's fan-in explodes linearly with scale, while a layered tree can press each node's fan-in to a constant;
- With the same monocular image, adding multi-view pose constraints can raise the coverage of novel-view prediction from 0.7456 to 0.9957.

These phenomena lie on none of the three axes of parameters–data–compute; they occur in the dimension between the axes: **how you connect the nodes, how you arrange the information, how you layer the modules.** This paper names this dimension the structure axis and argues it is the "fourth axis" orthogonal to the three axes.

### 1.3 Position and contributions of this paper

This paper is the flagship synthesis paper of the whole volume, at a standard above the other 12 topical papers. Its contribution is not to run one more experiment but to:

1. **Give a falsifiable central claim and four explicit refutation conditions** (Chapter 2), converting the correct but empty phrase "structure matters" into a scientific claim that can be killed by experiment;
2. **Connect natural-science and engineering evidence** (Chapters 3, 4), placing the four-chapter natural history of chemical bonds–carbon chains–proteins–neurons and the measured figures of the UDOS engine together on one master table;
3. **Propose a unified framework** (Chapter 5): the structure–dynamics–energy ternary model and a "theorem-like statement" of cross-substrate isomorphism;
4. **Honestly face the objections** (Chapter 6), fully listing why structure is not a sufficient condition and when "structure" degenerates into an unfalsifiable all-purpose explanation;
5. **Give an executable research program** (Chapter 7), marking the gate roadmap from CPU prototypes to GPU and real data.

### 1.4 Reading route

Chapter 2 is the skeleton of the whole paper and should be read even by non-specialist readers—it defines what exactly we are talking about. Chapters 3 and 4 are evidence and readers may choose by interest. Chapter 5 converges the evidence into a framework. Chapter 6 is this paper's self-defense and the landing point of academic honesty. Chapters 7 and 8 face the future. Chapter 9 lists threats and validity, and Chapter 10 closes. The appendix gives reproduction indices for all measured figures.

---

## 2 Terminology and formalization: what "structure" means and what it is not

### 2.1 The triple definition

This paper defines a system's **structure** as the triple $S=(T, G, M)$:

- **$T$ connectivity topology**: which units in the system have connections, and the direction and type of connections. It answers "who relates to whom." It does not care how strong a connection is, only whether it connects.
- **$G$ geometry/spatial organization**: the metric and orientation of connections in an embedded space—distance, pose, angle, the numerical values of fan-in and fan-out. It answers "with what strength, in what direction, at what position things relate."
- **$M$ modular boundaries**: into which submodules the system is divided, how levels nest, and where the interfaces and encapsulation boundaries between modules lie. It answers "which part counts as a whole."

![Figure 1 Conceptual diagram of the structure triple](figures/P0_fig1_structure_triad.png)

> Figure 1 Conceptual schematic of the structure triple. The three sub-dimensions—connectivity topology, geometry/spatial organization, modular boundaries—together constitute "structure" and are clearly distinguished from "parameters" and "dynamics" below. Schematic, not measured data.

These three sub-dimensions must coexist; missing any one does not constitute complete "structure." A pure graph (only $T$, no distances or levels) is insufficient to describe a protein: two different folded chains, even if their connection order (topology) is the same, are different isomers with different functions if their geometric arrangement in 3D space differs. Conversely, a system without modular boundaries—flattening all units into an undifferentiated net—cannot be hierarchically designed or verified even if topology and geometry are clear.

### 2.2 Strict distinction: structure ≠ parameters ≠ dynamics

The whole argument of this paper rests on the strict separation of three concepts. This separation is the premise of falsifiability—only when we can fix two and change the third is "structure matters" a testable proposition.

- **Parameters**: connection strengths, the chemical composition of matter, model weights. It is "how strongly things connect," "with what material they connect." Parameters are replaceable, learnable, and can be added or removed.
- **Dynamics**: how signals propagate over connections, how energy flows, how learning rules modify connections. It is "what rules run over the connections."
- **Structure**: whether connections exist ($T$), the metric and orientation of connections ($G$), how modules are divided ($M$). It is "the skeleton of connections itself."

The relation among the three can be given by an intuitive analogy: **structure is the topology and spatial wiring of a circuit diagram, parameters are the resistance values and materials of the components, dynamics is how the current flows and how the components age once powered on.** You can replace copper wires with silver wires (change material/parameters) and raise the voltage (change energy/dynamics), but if you change the circuit diagram itself—for example replacing a bridge circuit with a series one—the circuit's function changes even if the components are identical. This distinction is used repeatedly below, especially in the controlled comparisons of Chapter 4 where "only topology changes, parameters and compute fixed."

It is worth specifically noting that deep-learning research history has long had a tradition of conflating the three. Early work mixed layer count, width, and activation functions together under "architecture," treating "architecture search" and "hyperparameter tuning" as the same thing; when attention was proposed, people first thought it was a new "parameterization method," only later gradually seeing that what it really changes is the topology of "whether and how any two points connect." This paper argues for unpacking this historical conflation: when a researcher says "changing the architecture made it stronger," one must further ask—did they change connectivity topology $T$, embedded geometry $G$, or modular boundaries $M$? If none of the three can be stated, then "architectural progress" remains at the stage of engineering metaphysics. The three-way terminology of this paper exists precisely to make such questioning operational. One more point: between parameters and structure there is an often-overlooked intermediate state—**the existence of a connection can be learned parameterically** (attention weights are "dynamically determined connection topology"), which is precisely where deep learning advances beyond hand-built circuits; but this only shows $T$ can be shaped by $D$, not that $T$ and $D$ are the same thing. Section 6.4 returns to this coupling.

### 2.3 The central claim

**Central claim (C):** given fixed components and a fixed energy budget, the intelligent behavior a system exhibits is primarily determined by its spatial relational structure $S=(T,G,M)$; $S$ is the invariant across substrates (molecules–cells–neurons–machines–agent collectives), and materials and substrates are replaceable implementations.

Stated more operationally: there exists a class of tasks in which two systems $A$ and $B$ have essentially the same parameter count, material composition, and energy consumption, the only difference being $S_A \neq S_B$; if $A$ and $B$ show order-of-magnitude differences in task performance, and that difference cannot be remedied by adding parameters to $B$, then claim C is supported.

This is a strong claim, but it is strictly constrained by the refutation conditions of the four predictions below.

### 2.4 Four falsifiable predictions

The value of a scientific proposition lies not in how much it can explain but in what can kill it. For each of the four predictions below, "what experimental result would refute this paper" is explicitly stated.

#### Prediction 1 (topology trades for capability, same parameters and compute)

**Statement:** in two systems with exactly the same total parameters and total compute budget, changing only the connectivity topology $T$ (for example replacing a star with a layered tree, or with a centerless swarm) produces order-of-magnitude differences in coordination capability, and such differences do not disappear with "larger parameters."

**UDOS measured prototype** (reports7/matrix_scale_demo.json, cpu-proto): at million-level agent scale (target 1,000,000, actual 2,396,745 units), the star center's fan-in reaches **20,971,520** and the critical-path rounds reach **8,388,608**; the layered tree presses each node's maximum fan-in constantly to **9** and the critical-path rounds to only **14**. Parameter count and task volume are exactly identical; only topology differs.

**What result would refute Prediction 1:** if in strictly same-parameter, same-compute GPU/real-machine comparisons, the coordination success rate and convergence time of star and layered on the same batch of tasks fall in the same order of magnitude (for example fan-in differs by three orders of magnitude while success rate differs by less than 10%), and that conclusion stably reproduces across multiple seeds and task families, then "topology causes order-of-magnitude capability differences" is falsified, and this paper degrades to the weaker claim that "topology only affects efficiency, not capability."

#### Prediction 2 (breaking structure while preserving parameters; adding parameters cannot compensate)

**Statement:** artificially breaking structure (randomly rewiring edges, scrambling view poses, erasing the handoff bundle) while keeping parameter count fixed significantly degrades system performance; at that point simply increasing parameters cannot restore performance.

**UDOS measured prototype** (the estimator_param_error section of reports7/v7_verification.json, main gate verified, estimator a cpu-proto pilot point estimate): when structural information is insufficient (the observation window too short, equivalent to "structure broken"), the identifiability skill for the dynamics parameter acceleration $a$ is **−0.508**—worse than predicting with the mean; when structure (window/kinematic structure) is sufficient, the skill for the same type of parameter rises to +0.82 (other_v2). Structure determines "whether this thing is knowable."

**What result would refute Prediction 2:** if on the structure-broken experimental group, enlarging parameters 5–10-fold stably restores performance to the same level as the unbroken group (the error gap disappears to insignificance), then "structural information cannot be compensated by parameters" is falsified—meaning large models can hard-learn the broken structure through capacity, and structure's independent causal standing does not hold.

#### Prediction 3 (structure correct, substrate replaceable)

**Statement:** as long as structure $S$ remains isomorphic, switching the implementation substrate from one to another (carbon-based molecules → silicon circuits, biological neurons → artificial neurons, simulated agents → real robots) can preserve the system's core function.

**UDOS measured prototype** (reports7/splat_transfer_demo.json, cpu-proto): when the geometric structures of Real-to-Sim and Sim-to-Real are consistent, the robust-transfer strategy has success rate 1.0 and collisions 0.0, with significant margin (margin = 0.22) over naive transfer's 0.5/0.5. This is a small but real piece of evidence that "spatial geometric structure is preserved across real/simulated substrates."

**What result would refute Prediction 3:** if there exists a class of repeatedly verified tasks whose core function strongly depends on the physical properties of a particular substrate (for example a cognitive function that only emerges under the specific diffusion time constant of wet chemical synapses and cannot be reproduced in silicon electronic circuits no matter how structure is adjusted), then "cross-substrate isomorphism" is falsified, and this paper must retreat to the narrower claim that "structure is transferable only within a certain substrate family."

#### Prediction 4 (the threshold of structure prior to learning: before structure reaches the threshold, gains from adding data/parameters tend to zero)

**Statement:** there exists a topology/structure threshold. Before structure reaches that threshold, the marginal gains from increasing data volume and parameter count rapidly tend to zero; only once structure crosses the threshold is the parameter–data scaling activated. Structure is the "fourth axis" of scaling.

**UDOS measured prototype** (reports7/agent_scaling.json, k≤2 verified, k>2 cpu-proto): ensemble quality saturates with agent count $k$ as $mse(k)=a+b/k$, with fit $a=0.009252$, $b=-4.23\times10^{-5}$; when newly added members are only near-correlated copies of the strongest expert (structurally introducing no new connections/information sources), $b\approx0$ and gains go to zero. This echoes the "fourth structure axis" beyond Chinchilla's "parameters and data grow in proportion."

**What result would refute Prediction 4:** if on a topology clearly "wrong" structure (for example randomly connecting all experts into a fully connected noise graph, or changing a layered structure into a flat layer with no interfaces), simply enlarging parameters and data in Chinchilla proportions still lets performance keep falling along a power law with no saturation inflection, then the "structure threshold" is falsified—meaning structure is only a proxy for parameters, not an independent axis.

> The common refutation logic of the four predictions: this paper's central claim C is killed by the stable reproduction of any one of "topology does not affect capability," "structure can be compensated by parameters," "substrate is not replaceable," or "structure has no threshold." These four are not rhetoric; they are this paper's epitaph conditions.

---

## 3 Natural-history evidence (I): "structure determines function" at four scales

This chapter traverses four natural domains by a unified four-step template: **what structure is → how structure determines function → counterexamples of breaking/changing structure → isomorphic mapping to AI**. These four steps are the methodological master template of the whole paper, and Chapter 4 reuses the same template for UDOS evidence.

![Figure 2 Isomorphic comparison table of the four natural-history domains](figures/P0_fig2_natural_isomorphism.png)

> Figure 2 Comparison of the "structure → function → counterexample" chains across the four natural domains. Each row's structure differs, but the abstracted causal shape is consistent: components unchanged, only structure changes, and function changes. Schematic.

### 3.1 Chemical bonds and materials: isomerism is the cleanest natural experiment

**What structure is.** Starting from the quantum-mechanical explanation of chemical bonds, Pauling, in the first 1939 edition of *The Nature of the Chemical Bond*, systematically established a causal chain: chemical bonds determine molecular structure, and molecular structure determines material behavior (Linus Pauling research archive, Lindau Science Library; verified 2026-09-19). Bond types (ionic, covalent, metallic), the spatial arrangement of the crystal lattice, and atomic positions within the unit cell together determine a material's macroscopic properties: graphite and diamond, both pure carbon, one soft and conductive, the other hard and insulating, differing only in whether the carbon atoms are arranged in layers or tetrahedrally bonded. This is the most direct example of "connectivity topology + geometric organization" determining physical properties.

**How structure determines function.** The basic belief of materials science is: macroscopic properties (hardness, conductivity, melting point, reactivity) are not a direct function of "what elements a material is" but of "how those elements are arranged." The same iron, arranged body-centered cubic versus face-centered cubic, has very different toughness and strength.

**Counterexamples of breaking/changing structure—isomerism.** The key evidence of this section is isomerism. Structural isomers have exactly the same molecular formula (the same atom types and counts, i.e. the same "components/parameters"), and merely because the connection order of atoms differs, they become different substances with very different physical and chemical properties. Taking butane $\mathrm{C_4H_{10}}$: n-butane is a straight chain and isobutane (2-methylpropane) is branched, with different boiling points; positional isomerism moves a functional group one position along the carbon chain and reactivity changes; functional-group isomerism turns an alcohol (e.g. ethanol $\mathrm{C_2H_5OH}$) and an ether (e.g. dimethyl ether $\mathrm{CH_3OCH_3}$, same formula $\mathrm{C_2H_6O}$) into two completely different classes of compounds. This is the cleanest controlled experiment in nature: **components identical, parameters identical, the only variable is connection structure, and the result is vastly different function.** It directly supports the "under the same components structure determines function" side of central claim C.

**Isomorphic mapping to AI.** This is fully isomorphic to "isomerism" in model design: two neural networks with the same parameter count and training data, merely because of different connection topologies (dense connections vs sparse routing, residual vs direct, layered vs flat), can differ order-of-magnitude in capability and efficiency. The fan-in difference of UDOS star vs layered in Section 4.2 below is the multi-agent version of "n-butane vs isobutane."

The diamond-and-graphite example must also be added, because it exposes especially clearly that "geometry/spatial organization $G$ also independently determines function." Diamond and graphite are both pure carbon, and their atomic connection topology (carbon atoms bonding to each other) is almost identical stoichiometrically; the difference is only in how the carbon atoms are arranged in 3D space: in diamond each carbon bonds tetrahedrally to four neighbors, forming a rigid 3D network, so it is hardest and non-conductive; in graphite carbon is arranged in in-layer hexagonal grids, with layers stacked only by weak van der Waals forces, so it is soft, serves as a lubricant, and conducts along the layer direction. **The same material, almost the same bonding, merely because the 3D arrangement (geometric organization $G$) differs, the physical properties are as different as cloud and mud.** This example is especially important to this paper: it shows that even fixing $T$ (connections) and changing only $G$ (spatial arrangement), function changes drastically—$G$ is not a vassal of $T$; it is an independent dimension in the triple. Section 4.3 below, "multi-view pose structure determines novel-view prediction," is precisely the engineering counterpart of $G$ acting independently.

### 3.2 Carbon chains and life: structural organization precedes special substances

**What structure is.** Carbon's tetravalence lets it form chains and rings and build skeletons; the spatial arrangement of functional groups on the skeleton determines a molecule's chemical behavior. Life is possible not because the element carbon has anything mysterious—silicon, nitrogen, and phosphorus can also bond—but because the structural organization of "carbon tetravalence forming chains + functional groups being replicable and mutable" happens to support an information-storage and replication mechanism that is "heritable, mutable, evolvable."

**How structure determines function.** Watson and Crick's 1953 *Nature* paper *A Structure for Deoxyribose Nucleic Acid* (Nature 171(4356):737–738, 1953-04-25; awarded the 1962 Nobel Prize in Physiology or Medicine) revealed: DNA's replication ability comes directly from its double-helix structure—two complementary strands wound by A-T, C-G base pairing, with one strand's sequence uniquely determining the other. The paper's famous line "it has not escaped our notice" says precisely: once the structure is seen, the replication mechanism is foreseen. **It is structure itself, not what rare material DNA uses, that makes heredity possible.**

**Counterexamples of breaking/changing structure.** Changing carbon-chain structure without changing components makes life collapse or mutate: reversing the spatial orientation of a chiral center (enantiomers), mirror molecules often have very different pharmacological activity in the body—one enantiomer is a medicine and the other may be toxic (the thalidomide event is the classic lesson of chiral-drug tragedy; its precise pharmacological details belong to accepted medical history and this paper does not expand exact doses). This shows: even mirror-flipping one sign in 3D geometry changes function drastically—geometric organization $G$ is no optional decoration.

**Isomorphic mapping to AI.** This chapter's implication for AI is profound: **life originates from structural organization, not some special substance.** Extrapolating to intelligence: if intelligence is also a "structural organization," then in principle it can be realized on any substrate that can support such relations—this is precisely the natural-history basis of Prediction 3 (substrate replaceable). Carbon-based is no more a privilege of intelligence than it is a privilege of life.

### 3.3 Proteins: sequences fold into shapes, and shape is function

**What structure is.** Proteins have four levels of structure: the primary is the linear amino-acid sequence, the secondary is local folds such as the α-helix/β-sheet (Pauling, Corey, and Branson correctly proposed the α-helix in 1951, the spring-like structure of about 3.6 residues per turn maintained by intra-chain hydrogen bonds), the tertiary is the 3D folded shape of the whole peptide chain, and the quaternary is how multiple peptide chains assemble. Pauling, for his contributions to chemical bonds and molecular structure, was called by Crick one of the "fathers of molecular biology."

**How structure determines function.** Anfinsen's denaturation–renaturation experiments on ribonuclease proved: all the information needed for a protein's 3D active conformation is already contained in its primary amino-acid sequence—placing the denatured protein back under appropriate conditions makes it spontaneously fold back into the active native conformation. This is called the Anfinsen principle (thermodynamic hypothesis), and Anfinsen was awarded the 1972 Nobel Prize in Chemistry, with his summary lecture *Principles that Govern the Folding of Protein Chains* published in Science 181:223–230 (1973, PubMed 4124164). Function is determined by "shape": an enzyme's catalysis relies on the geometry of its active site (the lock-and-key model and the later induced-fit model), and allostery relies on a structural change at one site transmitted through the folded shape to a distant site. **The protein does not "know" what it is catalyzing; what its binding site looks like determines what it grabs.**

**Counterexamples of breaking/changing structure—denaturation.** Protein denaturation is the cleanest counterexample of "structure broken, components unchanged": heating or adding urea breaks hydrogen bonds, and not one of the peptide chain's primary sequences (amino-acid connection order and composition) is lost, but the 3D fold comes apart and activity is lost. The elegance of Anfinsen's experiment lies precisely here—he reversibly broke apart and restored this structure, proving information is in structure and function relies on structure. More recent counter-evidence comes from AlphaFold: Jumper et al. 2021 in *Nature* published *Highly accurate protein structure prediction with AlphaFold* (Nature 596(7873):583–589, DOI:10.1038/s41586-021-03819-2), reaching median backbone accuracy 0.96 Å r.m.s.d.95 at CASP14. **Predicting structure is predicting function**—because as long as the shape is right, binding and catalytic functions are almost locked in. This is the strongest engineering confirmation in the AI era of the proposition "shape = function."

**Isomorphic mapping to AI.** The protein's "sequence → fold → function" chain is highly isomorphic to the neural network's "weights → connection graph → behavior" chain: the former has information in the sequence and function in the folded shape; the latter has parameters in weights and function in connection geometry. AlphaFold uses pure computation to predict folding from sequence, just as we hope pure computation can predict an agent system's coordination capability from connection topology. Prediction 2's "breaking structure means losing function" is denaturation for proteins; "adding parameters cannot compensate" is for proteins "lengthening the peptide chain by ten thousand amino acids cannot restore the already-scattered active-site shape."

The two mechanisms of "shape determines function" also need to be stated more precisely, otherwise they are easily read as a slogan. First is **lock-and-key and induced fit**: an enzyme's active site is not a passive pocket but undergoes local conformational change upon substrate binding (induced fit)—i.e. function comes from the geometric matching of site shape and substrate shape, with binding specificity almost entirely determined by "whether the geometry fits." Second is **allostery**: when a distant site binds a small molecule, it transmits the signal to the active site through geometric distortion of the whole folded chain, changing its activity. Allostery is the ultimate "structure transmits function"—information crosses segments not through new chemical bonds but only through the chained deformation of 3D shape. Together these two show: a protein's "intelligence" (selective recognition, remote regulation) is written entirely in its geometric structure, not on any "computing unit." The reason AlphaFold's revolution shocked all of biology is that it turned the "sequence → structure" step computable—and once structure is predictable, the whole functional chain of binding, catalysis, and allostery becomes inferable. This paper carries this logic to AI: if "structure → function" holds for proteins, then for agent systems, "connection topology → coordination capability" should likewise be computable and predictable.

### 3.4 Neurons and the brain: intelligence lies not in the unit but in the connectome

**What structure is.** Cajal, through silver staining around 1888, proved: the nervous system is made of discrete neurons (the "neuron doctrine," the term named by Waldeyer in 1891, the word synapse named by Sherrington), and neurons transmit signals through contact regions (synapses) rather than forming a continuous net; Cajal shared the 1906 Nobel Prize in Physiology or Medicine with Golgi, who held the "reticular theory." The revolutionary point of this discovery is: **it rewrote "what the nervous system is" from a continuous mass of jelly into a graph of discrete nodes connected by specific connections.** The location of intelligence was, for the first time, precisely pointed to "how the nodes connect."

**How structure determines function.** Hebb, in *The Organization of Behavior* (New York: Wiley, 1949), proposed: when neuron A's axon repeatedly and persistently participates in firing neuron B, A's transmission efficiency to B strengthens—later generations condensed this idea into "neurons that fire together wire together," forming a "cell assembly": a group of neurons repeatedly co-activated forms a structure as the physical carrier of an idea or memory. Note that "fire together, wire together" is a later mnemonic condensation, and Hebb's own original statement is a more cautious statement about synaptic-efficiency change. This mechanism shows: **on the biological side, learning is essentially shaping connection structure**—dynamics (firing rules) in turn shape structure (connection weights and patterns).

**Counterexamples of breaking/changing structure—connectome lesions and spatial encoding.** Breaking connections while preserving cells makes function lost: in brain injury, even if surviving neurons themselves are intact, as long as key connection bundles are cut, the corresponding cognitive function is lost—direct counter-evidence that "intelligence lies in connections, not units." The spatial-cognition example is especially precise. Tolman in 1948 in *Psychological Review* 55(4):189–208 (DOI:10.1037/h0061626) proposed the "cognitive map": a rat running a maze does not memorize a string of stimulus–response actions but internally builds a spatial-structure representation of the environment, so it can take shortcuts never traveled. In 1971 O'Keefe discovered "place cells" in the hippocampus—specific neurons fire when the rat reaches a specific location; in 2005 May-Britt Moser and Edvard Moser discovered "grid cells" in the entorhinal cortex—firing positions tile a periodic hexagonal grid in the environment, forming an internal coordinate system. O'Keefe and the Mosers were awarded the 2014 Nobel Prize in Physiology or Medicine. **The brain encoded "spatial structure" itself into neuronal connections and firing patterns**—this is the hardest landing in neuroscience for this paper's title "spatial structure is itself intelligence."

**Isomorphic mapping to AI.** This chapter's mapping to AI is all-around: the Transformer's self-attention is a "learnable connection topology" (Section 4.1); the hippocampus's place/grid cells prove that for a system to navigate in space it must explicitly encode "positional relations" into structure—mutually confirming the decisive role of UDOS Chapter 4's multi-view pose structure on novel-view prediction (Section 4.3). Hebb also reminds us of an important bidirectionality: learning shapes structure, and structure also constrains learning—this must be honestly handled in the "structure is both cause and effect" boundary of Chapter 6.

Two more neuron-side pieces of evidence are worth adding, because they directly support this paper's "structure" stance. First is **connectomics**: a century after Cajal proposed the neuron doctrine, modern connectomics systematically maps complete neural connection diagrams, with the basic belief—to understand what a nervous system is doing, knowing what neurons exist and how each fires is insufficient; one must know the complete connection diagram among them. This is fully consistent with this paper's claim that "intelligence lies in connections, not units." Second is **criticality**: many experimental and theoretical works point out that living neural networks seem to spontaneously self-organize into a "critical state"—neither quiet nor bursting, where signals can just propagate through the system without losing control. If this picture holds, then "whether structure organizes the system to criticality" is an independent structural-design problem, not a mere parameter-adjustment problem. This paper does not write these two as settled conclusions (connectomics' functional mapping remains early, criticality's experimental evidence remains disputed), only using them as evidence that "structure is seriously treated as an independent research object in neuroscience." When precise original literature is needed, please supplement dedicated reviews of connectomics and neural criticality [CITATION NEEDED].

> Natural-history summary: from chemical bonds to neurons, the same causal shape repeatedly appears at four scales—**components unchanged, only structure (topology/geometry/modules) changes, and function changes; breaking structure, even with parameters and energy present, function is lost.** Table 1 converges this shape into a table.

**Table 1 The four natural-history chapters' "structure → function → counterexample → AI mapping"**

| Scale | Protagonist of the structure triple | Function determined by structure | Counterexample of "components unchanged, structure changed" | Isomorphic mapping to AI |
|---|---|---|---|---|
| Chemical bonds/materials | Bond type + lattice geometry | Macroscopic physical properties | Isomerism (same formula, different connections, very different properties) | Capability difference of same-parameter different-topology networks |
| Carbon chains/life | Carbon tetravalence chains + functional-group positions | Replicable and mutable | Enantiomers (chirality flipped, pharmacology very different) | Structure can cross substrates (Prediction 3) |
| Proteins | Primary sequence → 3D fold | Binding/catalytic function | Denaturation (chain unbroken, fold scattered, activity lost) | Predicting structure is predicting function (AlphaFold) |
| Neurons/brain | Connectome + hierarchical modules | Memory/spatial navigation | Connection-bundle lesions (cells alive, function lost) | Attention = learnable connection topology |

---

## 4 AI and UDOS evidence (II): making structure a measurable engineering quantity

This chapter is the original-evidence core of this paper. Unlike the natural-history evidence of Chapter 3, the figures here come from the CPU deterministic prototype of UDOS Reasoning Engine v7.5.0 (commit 1a71270 series), all tracing back to the raw JSON in reports7/, marked by evidence level. The meaning of evidence levels must first be stated, lest figures be misread as "already verified on real large models/real machines":

- `verified`: results reproducible on fixed-seed deterministic CPU and with key assertions passing contract tests;
- `cpu-proto`: point estimates on a CPU prototype, with the mechanism shape credible but constants not calibrated across multiple seeds and real machines;
- `cpu-proxy`: replacing real processes with synthetic proxies (e.g. asyncio.sleep simulating remote I/O, fixed-seed perturbation simulating real noise);
- `unverified`: external industry figures or media retellings, used only as background, not mutually verified.

**General rules for experiments and ablations (AlexNet-style "cost of removing X").** The four core measurements of this chapter follow one discipline: **whenever claiming "some structure matters," one must give the "cost of removing/breaking that structure," not only report "adding it is better."** The way AlexNet proved depth cannot be deleted is "removing any convolutional layer ≈ −2% top-1"; the counterparts in this chapter are: (a) P1 star vs layered—at the same $N$ the star center's fan-in explodes (infeasible) while layered presses to constant 9, this being "the cost of removing layered structure (retreating to star)"; (b) P8 single-view vs multi-view—removing pose geometry $G$, IoU falls from 0.8897 back to 0.7147; (c) P7 pure-semantic/pure-motion vs layered—removing modular boundaries $M$ (no handoff), direct contact tasks 0%, motion ordered-sorting 0%; (d) P6 short window—removing observation structure, acceleration skill=−0.508 (worse than the mean). Each group is read by "**experimental setup → comparison → ablation/sensitivity → validity threat**": base/seed/data volume are given below, the comparison is the same-budget baseline, the ablation removes some structural variable, and validity threats are concentrated in Chapter 9.

### 4.1 Transformer / Attention: the content–position–relation sequence of structural priors

**What structure is.** The Transformer established by Vaswani et al. 2017 in *Attention Is All You Need* (NeurIPS 2017, arXiv:1706.03762) essentially transforms "connection topology" from the RNN's temporal hard connections and the CNN's local hard connections into a **learnable connection topology**: self-attention lets any two positions in a sequence interact directly, with connection weights computed dynamically from content. There is an often-overlooked structural hierarchy here—content (token representation), position (positional encoding), and relation (attention matrix) are three different things, and the attention matrix is a learnable graph of "who relates to whom and how strongly."

**How structure determines function.** Self-attention's success is not because it has "more parameters" but because it changes long-range-dependency connections from "must pass through intermediate layers one by one" to "any two points directly connected"—precisely a change in connectivity topology $T$. MoE (Mixture-of-Experts) further pushes "trading structure for capacity" to the extreme: Fedus, Zoph, and Shazeer's Switch Transformer (JMLR 2021, arXiv:2101.03961) uses top-1 sparse routing, making total parameters about 1.6T and training about 7× faster, but each forward activates only a small fraction of experts—**total parameters did not increase; the routing structure of "which parameters are connected into this computation" changed.** This is the textbook case of "trading structure for capacity": parameter count is appearance, routing structure is what is designed.

**Position relative to this paper's claim.** Transformer and MoE together show: a significant part of modern large-model progress occurs on the "structure axis," not the pure parameter axis. This provides the first engineering-side support for the "fourth structure axis" of Chapter 8.

It is worth unpacking the three layers of "content–position–relation" clearly, because they correspond exactly to this paper's structure triple. In self-attention, for a token to be understood, three types of information are needed: content (who this token itself is, provided by embedding), position (where it is in the sequence, provided by positional encoding), and relation (what connections and how strong between it and every other token in the sequence, provided by the attention matrix). These three are not the same: you can fix content and position (the same sentence, same order) and only recompute the attention relation matrix, and the model's behavior changes; you can also fix attention relations (which two tokens attend to each other) and only change content representation, and behavior changes again. **The relation layer (attention matrix) is the connectivity topology $T$ this paper speaks of, except it is learnable and dynamic.** This is extremely important to this paper: it shows modern large models did not discard "structure" but turned "connectivity topology" from hand-designed hard connections into soft connections dynamically determined by content—structure did not disappear, it was learned parameterically. This is in principle the same as Hebb's "co-firing shapes connections": connection strength is determined by usage history. Section 6.4 below, when discussing "structure is both cause and effect," takes attention as the most modern example. MoE's sparse routing designs structure in another direction: it does not let connections be dynamically learned but pre-designs on a static architecture the topology of "which tokens should be routed to which experts"—the convergence of hand-designed and learnable structural design.

### 4.2 P1: changing only topology, coordination capability differs by six orders of magnitude (direct evidence for Prediction 1)

This is the core measurement of Prediction 1. Data are taken from reports7/matrix_scale_demo.json (evidence level cpu-proto; the first two levels 10, 100 are explicit simulations and consistent with closed-form formula values explicit_matches_formula=true, the latter four levels 1,000 to 1,000,000 are analytical closed-form extrapolations).

Under exactly the same task (the layered coordination matrix of 24 work orders), changing only the connection topology from "star" to "b-ary layered tree," two structural quantities are measured: center fan-in (the number of messages the center node must receive instantaneously) and critical-path rounds (the message round-trip steps needed to complete coordination). Results are in Table 2.

**Table 2 UDOS topology six levels: star vs layered (only $T$ changed, parameters and tasks unchanged)**

| Target agent count | Actual unit count | Depth | Star center fan-in | Layered maximum fan-in | Star rounds | Layered rounds | Evidence/level |
|---|---|---|---|---|---|---|---|
| 10 | 73 | 2 | 640 | 9 | 256 | 4 | explicit / cpu-proto |
| 100 | 585 | 3 | 5,120 | 9 | 2,048 | 6 | explicit / cpu-proto |
| 1,000 | 4,681 | 4 | 40,960 | 9 | 16,384 | 8 | analytical / cpu-proto |
| 10,000 | 37,449 | 5 | 327,680 | 9 | 131,072 | 10 | analytical / cpu-proto |
| 100,000 | 299,593 | 6 | 2,621,440 | 9 | 1,048,576 | 12 | analytical / cpu-proto |
| 1,000,000 | 2,396,745 | 7 | **20,971,520** | **9** | **8,388,608** | **14** | analytical / cpu-proto |

![Figure 3 UDOS topology fan-in/rounds double-logarithmic chart](figures/P0_fig3_topology_fanin_rounds.png)

> Figure 3 Horizontal axis = target agent count $N$ (log scale, 10→10⁶), left vertical axis = center/maximum fan-in, right vertical axis = critical-path rounds. Red dots: star explodes linearly ($\beta=1$) with $N$ (at N=10⁶ fan-in 20,971,520, rounds 8,388,608); cyan/purple lines: layered presses fan-in to constant 9 and rounds to log level (at N=10⁶ only 14 rounds). Baseline = same $N$ and task volume, only topology $T$ differs. Data trace to matrix_scale_demo.json; evidence level: cpu-proto (first two levels explicit, latter four analytical extrapolations).

Reading the figure: at million-level scale, the star center node must simultaneously receive about 20.97 million messages (center fan-in), and completing coordination needs about 8.39 million critical-path rounds; the layered tree makes each node's maximum fan-in constantly 9 (except the root, with 8 subordinate units), and the critical path needs only 14 rounds. **Parameter count, task volume, and energy budget are exactly identical; the only variable is connectivity topology $T$, and coordination overhead differs by about six orders of magnitude.** This is the direct prototype of Prediction 1 in this repository.

It must be honestly stated: the P1 paper itself also notes this repository did not measure "the collapse comparison of a pure star under center failure," so the fault-robustness side of "layered outperforms star" remains a planned experiment; Table 2 measures the structural observables of **coordination complexity (fan-in, rounds)**, not end-to-end task success rate. Translating fan-in/rounds into real latency and success rate needs measurement in a real distributed environment—this is the GPU/real-machine gate of Chapter 7.

### 4.3 P8: multi-view pose structure determines novel-view prediction (evidence for geometry $G$)

**What structure is.** Novel-view synthesis solves: from images of known views, predict what an object looks like from a never-seen view. The structural variable here is **camera geometry and multi-view pose constraints**—i.e. the relative pose relations of observation views in 3D space (geometry/spatial organization $G$).

**Measurement.** Data are taken from reports7/spatial_novelview_demo.json (v7.3.8, cpu-proto, TSDF explicit voxel representation, 8 training views, 6 held-out views, voxel 0.1m). Comparing "single view" with "multi view with pose structure":

**Table 3 P8: the effect of multi-view pose structure on novel-view prediction (cpu-proto)**

| Metric | Single view | Multi view (with pose structure) | Change |
|---|---|---|---|
| Contour IoU | 0.7147 | 0.8897 | +0.1750 |
| Coverage | 0.7456 | 0.9957 | +0.2501 |
| Precision | 0.9472 | 0.8931 | −0.0541 |
| Depth MAE (m) | 0.3186 | 0.0924 | −0.2262 |

Reading the figure: merely adding the "relative pose structure of multiple views" (parameters and training data essentially unchanged), held-out views' contour IoU rises from 0.7147 to 0.8897, coverage from 0.7456 to 0.9957, and depth error falls to about one-third. The report's own conclusion is: "multi-view spatial context's contour IoU/coverage on held-out views is significantly higher than single view, proving novel-view prediction relies on camera geometry and 3D consistency."—precisely direct evidence that geometry/spatial organization $G$ determines prediction capability, isomorphic to the hippocampus's place/grid cells encoding spatial relations into structure (Section 3.4). splat_transfer_demo.json (v7.3.9, cpu-proto) supplements: the CPU prototype of 150 3D Gaussians reaches novel-view IoU 0.7285 and coverage 1.0, with Sim-to-Real robust-transfer success 1.0/collisions 0.0 (vs naive transfer 0.5/0.5, margin 0.22)—spatial geometric structure preserved across simulation/reality, a small engineering piece of evidence for Prediction 3.

### 4.4 P7: the layered structure of cognitive layer × motion priors, 4.7% takeover rate for 100% success (evidence for modular boundaries $M$)

**What structure is.** In embodied control there have long been two schools: general large models are strong at semantic planning but weak at dense contact control; motion priors (diffusion policies etc.) are strong at high-frequency contact but weak at semantic order. P7's structural scheme is **layering**: the high-level semantic layer only scores the low-level motion-prior candidates when necessary, choosing between "adopt the prior" and "issue a short corrective takeover." This is a design of modular boundaries $M$ and levels—encapsulating "cognition" and "motion" into two independently replaceable modules, handing control by confidence.

**Measurement.** Data are taken from the P7 paper abstract and its source reports7/embodied_hybrid_demo.json (v7.3.7, cpu-proto, paired experiments of 4 tasks × 10 rounds, 40 rounds per mode):

**Table 4 P7: control mode × success rate/takeover rate/tokens (cpu-proto, pilot point estimate)**

| Mode | Aggregate success rate | Mean score | Collisions | LLM calls | Takeover rate | Token proxy (in/out) |
|---|---|---|---|---|---|---|
| Pure semantic direct | 75% | 76.97 | 0.25 | Every decision segment | 100% | 616k / 182k |
| Pure motion prior motion | 52.5% | 69.57 | 0 | Zero | 0% | 0 / 0 |
| Layered hybrid | **100%** | 100 | 0 | Only when necessary | **4.7%** | 331k / 40k |

Reading the figure: any single module alone is incomplete—direct is 0% on contact-door tasks, motion is 0% on ordered-sorting tasks; but once the two are assembled by modular boundaries through a **layered structure**, hybrid simultaneously gains both sides' capabilities, reaching 100% success and zero collisions with only 4.7% takeover (i.e. 95.3% of decision segments resolved by motion priors themselves), with token proxy volume also significantly falling. Task decomposition shows takeovers highly concentrated on tasks where "the semantic layer is truly irreplaceable." This is direct evidence that modular boundaries $M$ determine system capability: **not which module is stronger, but how you layer and hand off between them determines whether the whole can succeed.** It shares origin with SayCan (Ahn et al., 2022, arXiv:2204.01691)'s two-layer structure of "LLM gives step candidates × skill value function gives feasibility."

### 4.5 P6 counterexample: when structural information is insufficient, even adding parameters cannot identify (hard counterexample for Prediction 2)

This section is the most important reverse-direction evidence of the whole paper and the most direct support for Prediction 2: **structure determines "whether this thing is knowable."**

Data are taken from the estimator_param_error of reports7/v7_verification.json (v7.0.3, main gate verified, estimator part cpu-proto). The system-identification task must reverse-infer hidden dynamics parameters from observation data. Results are as follows:

**Table 5 P6: structure (observation window/observability) determines parameter identifiability (verified/cpu-proto)**

| Parameter to identify | MAE | Mean-prediction MAE | Identifiability skill | Mean observability | Evidence |
|---|---|---|---|---|---|
| Initial velocity v0 | 0.6606 | 0.9683 | 0.318 | 0.771 | cpu-proto (n=480) |
| Acceleration a (structure insufficient) | 0.9256 | 0.6138 | **−0.508** | 0.798 | cpu-proto (n=160) |
| Spring frequency ω | 0.1471 | 0.2563 | 0.426 | 0.914 | cpu-proto (n=160) |
| Other v2 | 0.0432 | 0.2396 | 0.820 | 0.908 | cpu-proto (n=160) |

Reading the figure: a negative identifiability skill means "our estimate is worse than directly predicting with the global mean." The acceleration parameter $a$'s skill = **−0.508**—because under the observation structure of a short window (window=6) it is **fundamentally unidentifiable**: although mean observability is not low (0.798), structurally acceleration cannot be observed from the given window. **At this point no matter how many parameters or model capacity you add, you cannot conjure information fundamentally not contained in the observation structure.** This is precisely the reverse confirmation of Prediction 2: when structure (here observation-window/observability structure) is missing, parameter compensation fails. The P5 world model under oracle structure blind test overall MSE falls from v701's 0.0459 to 0.01037, with all conformal coverage passing (the marginal coverage interval of the six-combination merge 0.7758–0.9577, nominal 0.8/0.9/0.95), showing once structure and the data flywheel are in place, capability is truly released.

The theoretical implication of this result is worth expanding, because it touches the more general proposition "structure determines knowability." System identification has a classic and cold conclusion: **whether a parameter can be estimated from data depends not on how strong a model you use to fit, but on whether your observations structurally "excite" that parameter's influence**—the core of observability and identifiability theory. If the observation structure (window length, sensor configuration, kinematic constraints) itself does not let acceleration leave distinguishable traces, then no matter how wide or deep you make the neural network, it can only fit the mean, and skill is naturally negative. UDOS's P6 measurement (skill=−0.508) is precisely a small engineering verification of this theory. Translating into this paper's language: **information availability is a structural property, not a parameter property.** This is fully consistent with "breaking structure cannot be compensated by adding parameters" (Prediction 2)—you cannot use a larger model to recover a degree of freedom that never appears in the observation structure. This is also why this paper repeatedly stresses "design structure right first": on a wrong observation/connection structure, more parameters only fit noise and the mean.

### 4.6 Other structural modules: handoff bundle, quorum, market conservation, state coverage, Stigmergy

The UDOS matrix lands structural principles on multiple mechanism modules; here each gets one set of measured figures as material for the master table.

- **P10 handoff bundle structure** (udos7/topology/transfer.py, agent_tool.py): handoff is not forwarding but responsibility transfer, with a fixed envelope + six-field validation; if validation fails the Owner does not change and responsibility does not slip away with the message. This is "the interface contract of modular boundaries $M$"—making cross-module handoff a structured, verifiable bundle rather than unstructured text forwarding.
- **P2 BFT-lite quorum structure** (udos7/topology/consensus.py): committee size $n\ge 3f+1$, quorum $2f+1$, same-round equivocation invalidates the whole round. At $n=7, f=2$ (the minimum legal committee with $3f+1=7$), the 7-person QA committee under 2 Byzantine counter-voting members correctly accepts all 24 work orders (24/24), with the faulty fleet triggering 5 reworks and isolating 3 faulty agents. The "structure" here is voting topology and majority threshold—it directly reuses the topology conclusion of classic BFT (Castro & Liskov, OSDI 1999, $f<n/3$).
- **P11 market conservation structure** (udos7/topology/market.py): the internal task market's budget is a conserved quantity. In matrix integration budget = paid = balance_sum = 240 (24 work orders × 10/order), conserved=true. Conservation is a structural invariant—it does not depend on which specific agent did which thing, only on the accounting structure of "where money comes from and goes."
- **P4 state-coverage structure** (reports7/egodata_flywheel_demo.json, v7.4.0, cpu-proto): the quality of active data collection depends on the coverage structure of the "state space." Oracle state units 226; passive collection covers only 157 (oracle ratio 0.695, yield 0.6667, redundancy 0.599); coverage-aware active collection covers 215 (oracle ratio 0.951, yield 0.8542). **Replacing passive data-piling with structured planning of "which state units to cover" raises oracle coverage from 0.695 to 0.951**—this is the structure of the data flywheel, not data volume.
- **Stigmergy environmental-medium structure** (udos7/topology/stigmergy.py, v7.4.10, cpu-proto): agents do not directly negotiate but atomically claim tasks and leave completion marks on a shared blackboard, letting the environment bear memory. On the 30-task/6-agent benchmark, stigmergy uses 2 traces per task for **60 messages total**; contract-net (broadcast–bid–award) uses about 8 negotiation messages per task for **240 total**. **Changing the "communication structure" from point-to-point negotiation to "environment bearing memory" reduces messages to about one quarter**—the result of jointly designing connectivity topology $T$ and modular boundaries (the environment as a shared module), isomorphic to Dorigo's ant pheromones (1996).

### 4.7 Master table: natural structure ↔ UDOS modules ↔ measured figures ↔ evidence levels ↔ common abstraction

**Table 6 Cross-chapter master table (this paper's evidence convergence)**

| Natural-structure case | UDOS corresponding module | Measured figures (trace reports7) | Evidence level | Common abstraction |
|---|---|---|---|---|
| Isomerism: components identical, structure differs | P1 star vs layered topology | Fan-in 20,971,520→9; rounds 8,388,608→14 | cpu-proto | Same parameters, only $T$ changes, capability order-of-magnitude difference |
| Protein geometry (shape = binding) | P8 multi-view pose structure | IoU 0.7147→0.8897; coverage 0.7456→0.9957 | cpu-proto | $G$ (pose geometry) determines prediction |
| Brain layering/modularization | P7 cognition × motion layering | 100% success; takeover 4.7% | cpu-proto pilot point estimate | $M$ (layered handoff) determines overall performance |
| Denaturation: structure lost, function lost | P6 short-window unidentifiability | Acceleration skill = −0.508 | cpu-proto (main gate verified) | Structure missing, parameters cannot compensate (Prediction 2) |
| Cell assembly/Hebb co-connection | P9 ensemble saturation mse=a+b/k | a=0.009252, b=−4.23e-5; equal-weight bad expert 0.2131 | k≤2 verified / k>2 cpu-proto | Homogenization (no new structure) expansion saturates |
| Connectome/synapse topology | P2 BFT-lite quorum | n≥3f+1, 24/24 accepted, 3 faulty isolated | cpu-proto | Voting topology determines safety/liveness |
| Conservation (metabolic structure) | P11 market settlement | budget=paid=240 conserved | cpu-proto | Structural invariant does not depend on individuals |
| Spatial cognitive map | P4 state-coverage flywheel | Oracle coverage 0.695→0.951 | cpu-proto | Coverage structure determines data value |
| Ant stigmergy | P1 blackboard-medium collaboration | 60 vs contract-net 240 messages | cpu-proto | Environment bears memory, reducing point-to-point negotiation |
| Cross-substrate (carbon/silicon) | P8/P8-splat Sim2Real | Robust transfer 1.0/0 collisions, margin 0.22 | cpu-proto | Geometric structure preserved across simulation/reality (Prediction 3) |

> Reading Table 6: every row says the same thing—measuring "structure" independently from parameters reveals its decisive contribution to system capability. The natural-history side (Chapter 3) and the engineering side (this chapter) meet end-to-end in this table.

---

## 5 Unified framework: the structure–dynamics–energy ternary model and cross-substrate isomorphism

### 5.1 The ternary model

Tightening the distinctions of Chapter 2 into a working model: any intelligent system is a triple

$$\text{System capability} = f(\,S,\,D,\,E\,) \tag{1}$$

where $S=(T,G,M)$ is structure (this paper's protagonist), $D$ is dynamics (learning rules, signal propagation, energy conversion), and $E$ is the energy budget. The three **multiply rather than add**—if any one approaches zero, system capability approaches zero:

- Only $S$ without $D$: a circuit diagram drawn on paper, a dead brain, a denatured peptide chain—structure present but not "alive," function zero;
- Only $D$ without $S$: a randomly stirred chemical soup, an unstructured fully connected noise network—dynamics present but relations not organized, unable to produce predictable coordination;
- $E$ insufficient: structure and dynamics both right, but energy cannot be supplied, the brain falls into coma, the model underfits.

The whole claim of this paper, while acknowledging this multiplicative relation, stresses that **when $D$ and $E$ are controlled (fixed), the marginal contribution of $S$ has been systematically under-estimated.** Chapter 6 repeatedly returns to this model to handle its counterexamples.

### 5.2 The theorem-like statement of cross-substrate isomorphism

Compressing the evidence of Chapters 3 and 4 into a testable "theorem-like statement" (note: this is not a mathematical theorem but a research-program statement awaiting rigorous proof):

> **Substrate-Isomorphism Conjecture:** suppose task $\tau$ has a "structure-sufficient solution" $S^*$. If two substrates $\mathcal{M}_1$ (e.g. carbon-based biology) and $\mathcal{M}_2$ (e.g. silicon machines) can both realize the triple $S^*=(T^*,G^*,M^*)$ at sufficient resolution, and dynamics $D$ and energy $E$ fall within the feasible interval of task $\tau$, then the two substrates' core functional performance on $\tau$ can be isomorphic. Substrate differences manifest in constants (speed, energy, precision), not in the presence or absence of function.

The value of this statement lies in its **clearly drawn falsifiable boundary**: it does not claim any substrate can run any structure (that is strong functionalism, which this paper does not endorse), only that "when structure is sufficiently realized and dynamics/energy are in the feasible interval," function can transfer across substrates. Prediction 3 is one testable version of this proposition. Section 6.3 discusses why "random structure also works" does not necessarily refute this proposition—because the proposition requires a "structure-sufficient solution," not "any structure."

### 5.3 The structure axis: the fourth axis omitted by scaling law

Existing scaling literature writes performance as a function of parameters $N$, data $D$, and compute $C$ (Kaplan 2020 power law; Hoffmann/Chinchilla 2022 corrected to $N$ and $D$ in proportion). This paper does not deny this three-axis curve but argues it implicitly freezes structure. UDOS's P9 measurement (agent_scaling.json) shows: when newly added agents are only near-correlated copies of the strongest expert—i.e. structurally introducing no new connections/information sources—ensemble quality saturates as $mse(k)=a+b/k$, with the saturation floor $a$ determined by the best single expert's quality ($a=0.009252$, close to best_single=0.009196). This shows: **when structure (independence among information sources/connection diversity) does not change, simply increasing "head count" (parameters/members) gains tend to zero.** This does not contradict Chinchilla's "parameters and data grow in proportion" but fills the omitted dimension: structure diversity. On the structure axis, "expansion" is not the more the better, but depends on whether you really introduce new connection structure—homogeneous expansion (structure unchanged) saturates, heterogeneous expansion (structure changed) keeps improving.

![Figure 4 Conceptual structure–parameter–performance surface](figures/P0_fig4_structure_parameter_surface.png)

> Figure 4 Conceptual surface schematic. Horizontal axis parameters/compute, vertical axis the structure axis (topology maturity), upright axis intelligent performance. Before structure crosses the topology threshold (red threshold line), marginal gains from adding parameters tend to zero; after crossing, parameter scaling is activated. Schematic, not measured data.

---

## 6 Counter-arguments and boundaries: what this paper does not claim

The honesty of a flagship paper is shown in whether it fully writes the counter-arguments. This chapter lists serious objections to central claim C one by one, along with our responses and concessions.

### 6.1 Structure is not a sufficient condition: dead brain, denatured protein, empty topology

The most direct objection is: **structure is only necessary, not sufficient.** An anatomically complete dead brain, connectome intact and neurons all present, but without metabolism and electrical activity ($D$ and $E$ zero), has no intelligence. A denatured protein, primary sequence (connection order) unchanged, only the 3D fold scattered—this instead shows "complete structure" requires geometry $G$ to also be present. A neural network with only a connection graph but no signals or learning rules, no matter how large its parameters, does nothing. **This paper fully accepts these counterexamples**: they precisely confirm the ternary multiplicative model of Section 5.1—structure is one multiplier, not the whole. This paper never claims "with structure there is intelligence," only that "with dynamics and energy fixed, structure is the dominant variable."

### 6.2 The functionalism vs substrate-neutrality debate

"Substrate replaceable" (Prediction 3) directly steps on the old debate in philosophy of mind between functionalism and biological naturalism. One side (strong functionalism) claims intelligence is entirely independent of substrate; the other (biological naturalism/embodied school) claims intelligence essentially depends on specific physical properties of biological wetware. This paper's stance is a moderate, testable middle ground: we do not take a metaphysical stance, only giving the falsifiable version of Prediction 3—"in task families where structure is sufficiently realized and dynamics/energy feasible, substrate differences manifest in constants." Section 2.4 already states what would refute it: if some cognitive function strongly depends on the specific time constant of wet chemical synapses and silicon cannot reproduce it, this paper retreats to "structure is transferable only within a substrate family." **We converted a metaphysical stance into an experimental question.**

### 6.3 Random structure also works: reservoir computing and the lottery hypothesis

A serious technical objection: randomly connected networks can also work. Reservoir computing / echo-state networks use random fixed recurrent connections and only train the output layer to complete time-series prediction; the lottery ticket hypothesis claims dense randomly initialized networks contain sparse "winning" substructures. If random structure works, how can one still say "structure is dominant"?

This paper's response is two-layer. First, the reservoir's "random" is random within a carefully designed structural family—the spectral radius of recurrent connections is tuned to criticality (guaranteeing the echo-state property), itself a deep structural constraint; it is not arbitrary randomness but "randomness designed to be critical." Second, the lottery hypothesis precisely supports this paper: it says "training filters a randomly initialized network into a winning substructure"—i.e. **the learning process keeps useful connection structures and prunes useless ones**, precisely evidence of structure being shaped. That random structure works does not mean structure is unimportant—it only shows good structures can be searched out. This instead leads to the next boundary.

### 6.4 Learning itself shapes structure: structure is both cause and effect

Hebb's "co-firing strengthens connections" reminds us: in biological neural networks, structure (connection weights and patterns) is itself a product of learning. This brings a causal-direction trouble—**structure is both the cause of intelligence and the result of learning.** How does this paper handle this loop? Our stance is: this does not refute the central claim, only refines it. Within a single system's lifecycle, learning shapes structure; but in cross-system comparison (Prediction 1: star vs layered) and cross-task design (Prediction 4: threshold), structure is an independently designable, independently measurable independent variable. UDOS's P1 experiment measures structural effects precisely under the controlled condition of "learning rules identical, only connection topology changed"—this controlled design temporarily separates "structure shaping" from "structure effects." Honestly, **the bidirectional coupling of structure and learning is a simplification of this paper's framework**, and complete unification requires bringing learning dynamics $D$ into the closed loop of structure evolution, an open problem of Chapter 7.

### 6.5 Description granularity: when "structure" degenerates into an unfalsifiable all-purpose explanation

This paper must self-guard against an epistemological trap: **the word "structure" is too useful, so useful it may become an all-purpose basket that explains everything and therefore explains nothing.** If any performance difference can be post-hoc attributed to "different structure," central claim C becomes unfalsifiable. This paper's defense has three parts: first, Section 2.1 pins structure to the triple $S=(T,G,M)$, each item with operationalizable measurements (fan-in, pose, module interfaces), not a vague "way of organizing"; second, all four predictions state explicit refutation conditions (Section 2.4), and this paper will be killed by specific experimental results; third, every row in the UDOS master table (Table 6) gives concrete figures and evidence levels, not allowing the word "structure" to cover unmeasured black boxes. **When "structure" cannot be reduced to any item of T/G/M nor give refutation conditions, it is an explanation that should be rejected.** This self-constraint, more than any positive argument, protects this paper from degenerating into metaphysics.

### 6.6 External validity of this repository: CPU prototypes are not LLMs, not real machines

Evidence levels must be stated thoroughly; this is the landing point of this paper's academic honesty. All UDOS measurements are **Linux CPU deterministic prototypes**: the semantic judge is deterministic rules rather than a real vision-language model; the environment is a point-mass double-integrator agent rather than MuJoCo-MJX contact simulation or a real machine; tokens are proxy accounting rather than real API billing; remote agents' I/O curves are simulated by asyncio.sleep rather than real LLM calls. The P9 paper itself also states: "all figures are CPU fixed-seed pilot point estimates, without multi-seed confidence intervals; their value lies in mechanism shape and failure boundaries, not precise constants." Therefore:

- The six-order-of-magnitude fan-in difference of Table 2 is **deterministic mathematics** (closed form consistent with explicit simulation), its "shape" credible; but translating it into real distributed systems' latency and success rate is the GPU/real-machine gate of Chapter 7;
- P7's 100% success is a pilot point estimate under a **synthetic point-mass environment**, not constituting a SOTA claim about real VLA models;
- P6's skill=−0.508 is measured on a **synthetically injected defect structure**, and real-world "structure missing" is more complex.

The claim strength of all engineering evidence in this paper does not exceed its respective evidence level. This is not modesty but the requirement of falsifiability—a proposition that dares not mark its evidence boundaries does not deserve to be called science.

### 6.7 Relation to scaling law: not denying, only adding an axis

Finally clarifying the relation to existing scaling literature to avoid being misread as "anti-scaling." This paper **does not deny** the Kaplan/Chinchilla parameter–data–compute three-axis curve; on the contrary, this paper treats it as a premise. This paper claims: those three axes were measured under "structure fixed" conditions—all use the Transformer, all stack parameters by established topology. Once structure is also made a design variable, a fourth axis appears (Figure 4). The test method this paper gives is the simple **same-parameter same-compute comparison**: fixing parameter count, compute, and data, changing only connection topology/module boundaries, to see whether performance jumps. If it does, structure is an independent axis; if not, this paper's Prediction 1 is falsified. This is not overthrowing scaling law but extending it from three dimensions to four.

---

## 7 Testable research program: the gate roadmap from CPU prototypes to GPU/real machines

This chapter turns the four predictions into executable experimental plans, each marked with a gate and current blocker.

**GPU gate for Prediction 1:** in a real multi-GPU distributed environment, using the same batch of real LLM agents (not CPU synthetic agents), run end-to-end task success rates and wall-clock latency for star vs layered topology, confirming whether the "six-order-of-magnitude fan-in difference" translates into "order-of-magnitude differences in success rate/latency." Current blocker: needs LLM keys and cloud budget (AL4 gate), which CPU prototypes cannot bear.

**Parameter-compensation gate for Prediction 2:** on the structure-broken experimental group (random rewiring, scrambled poses), enlarge parameters 5–10-fold to see whether performance recovers. Current blocker: real large-model parameter-expansion training needs GPU and compute budget; UDOS currently has only the 967,796-parameter world model (v7.0.3, verified), insufficient for large-scale parameter-compensation experiments.

**Real-machine gate for Prediction 3:** migrate the spatial geometric structure of P8/P8-splat from the CPU Gaussian-splat prototype to real depth-camera calibration, multi-view capture, and MuJoCo-MJX contact simulation, verifying "geometric structure preserved across simulation/reality." Current blocker: full 3DGS training and real-time rendering need GPU, real camera calibration and multi-view capture need hardware, real-machine Sim-to-Real needs GPU/HPC.

**Structure-threshold gate for Prediction 4:** under Chinchilla-style compute budgets, systematically scan the fourth dimension of "topology maturity," verifying whether a structure threshold exists—below it gains from parameters/data tend to zero, above it activated. Current blocker: needs large-scale training compute and multiple topology comparisons, far beyond CPU-prototype capability.

**Table 7 The gate roadmap of the four predictions**

| Prediction | Current evidence (CPU prototype) | Gate needed to unlock | Falsification observation |
|---|---|---|---|
| P1 topology trades for capability | Fan-in/rounds six levels (cpu-proto) | Real LLM agents + cloud GPU | Star/layered success same order of magnitude |
| P2 broken structure cannot compensate | Acceleration skill=−0.508 | Large-scale parameter-expansion training | Adding 5–10× parameters restores performance |
| P3 substrate replaceable | Sim2Real 1.0/0 (cpu-proto) | Real machine + MuJoCo-MJX | Some function strongly depends on wetware, non-transferable |
| P4 structure threshold | mse=a+b/k saturation (verified/cpu-proto) | Large-scale topology × compute scan | On wrong structure still falls along power law |

---

## 8 Discussion: architecture design, embodiment, multi-agent, and AGI

### 8.1 Implications for architecture design

If this paper's framework holds, then the architect's first-principles action should not be "choosing how large a model" but "designing the correct structure": how to topologize connections (avoiding center fan-in explosion), how to organize space (explicitly encoding pose and geometry), how to layer modules (cognition and motion, planning and execution handing off by confidence). UDOS's P1 (layered decoupling of fan-in), P7 (cognition × motion layering, 4.7% takeover), and P10 (handoff bundle as responsibility interface) jointly point to one engineering principle: **design structure right first, then discuss parameter scale.** This is in line with the post-Chinchilla industry awareness that "parameters are not the bigger the better," only pushing that awareness from "parameter-data ratio" to "structure itself."

More concretely, this principle has three operational implications for daily engineering decisions. First, **the fan-in budget should be written into design documents before the parameter budget**: if a center node must instantaneously receive tens of thousands of messages, no matter how large a model hangs behind it, it becomes a system bottleneck—P1 turned this intuition from "seems like a traffic jam" into computable fan-in/rounds upper bounds. Second, **geometry and pose should be first-class citizens, not attached byproducts of images**: P8 proved explicitly encoding multi-view relative pose can raise novel-view coverage to 0.9957; this means in embodied and 3D-system design, "how cameras are placed, how coordinates are aligned" is no preprocessing triviality but a structural variable determining the ceiling. Third, **module interfaces should have contracts like handoff bundles**: P10 and P7 both show cross-module handoff, once made into unstructured text forwarding, dilutes responsibility in forwarding; making handoff a verifiable, responsibility-bound bundle lets the system safely layer and scale. The common foundation of these three implications is the same belief—structure is an engineering object that can be designed, budgeted, and verified, not a byproduct emerging only after training.

### 8.2 Implications for embodied intelligence

Embodied intelligence's deepest lesson comes from neuroscience: the brain encoded spatial structure (place cells, grid cells) into connection patterns (Section 3.4). UDOS's P8 multi-view pose structure (Table 3) and P4 state-coverage flywheel (oracle coverage 0.695→0.951) reproduce the same principle on the engineering side—**embodied systems must explicitly encode "spatial relations and state coverage" into structure, not expect large models to figure it out themselves from raw pixels.** P7's layered hybrid control (Table 4) then shows: the embodied three-layer structure of "perception–planning–control" is no arbitrary engineering layering but a mirror of the biological cognitive architecture (hippocampus–cortex–cerebellum).

### 8.3 Implications for multi-agent systems

Multi-agent legions' most common narrative is "the more heads the stronger," but UDOS's P9 measurement (agent_scaling.json) gives a hard boundary: quality saturates as $mse(k)=a+b/k$, equal-weight averaging with one bad expert drags MSE to 0.2131 (far worse than calibrated weighting's 0.0092); throughput peaks at 2 concurrency 244.172 tasks/s then falls (4/8 concurrency down to 158.4/106.5). **The "expansion" dividend of multi-agent depends on whether you introduce new connection structure (information independence), not head count.** Combined with P1's layered topology, P2's BFT quorum, P11's market conservation, and Stigmergy's blackboard medium, UDOS gives a construction diagram of "how multi-agent should design structure": layering reduces fan-in, quorum ensures safety, market conservation manages incentives, blackboard reduces negotiation traffic.

### 8.4 Restrained implications for AGI

This paper's implications for AGI must be extremely restrained, avoiding sliding into big words. If "intelligence is a structural organization transferable across substrates" (Prediction 3), then a question worth studying is: **is there a structure isomorphic to carbon-based neurons but efficiently implemented on silicon?** This paper does not predict when AGI arrives nor claims structure alone leads to AGI. It only provides a research stance: rather than infinitely stacking parameters on a fixed structure (that axis's marginal gains already show saturation signs in P9), treat "finding the correct structure" itself as a searchable, measurable engineering problem. If AGI arrives, it will not be "a larger same network" but more likely "a new structure not yet designed"—this is the only, and restrained, hint this paper's framework gives to AGI research.

---

## 9 Threats to validity and limitations

> This section is this paper's independent validity-threat chapter. Chapter 6 handles "counter-argument philosophy" (whether structure is sufficient, the substrate-neutrality debate); this section, standing in the reviewer's position, lists the engineering-evidence threats by the five categories of internal/external/construct/statistical/causal—more honest than NSA (with no independent Limitations section in the whole text), and more reviewable than only mentioning "cpu-proto" in scattered paragraphs.

### 9.1 Internal validity: ablation purity and whether the "removal cost" is really caused by structure

- **Ablations are structural observables, not end-to-end success rates**: Table 2 measures fan-in/rounds (deterministic mathematics), not real task success rates. The P1 paper itself already stated "did not measure the collapse comparison under pure-star center failure," so the "layered is more robust" side remains a planned experiment.
- **Single-seed pilots**: P7's 100% success, P8's IoU improvement, and P6's skill=−0.508 are all single-seed, without 95% confidence intervals. **Needs ≥30 seeds before submission.**
- **Actively disclose biases adverse to oneself** (AlexNet footnote-2 honesty): the latter four levels of Table 2 (1,000→1,000,000) are analytical closed-form extrapolations, not explicit simulations; only the first two levels (10, 100) have explicit_matches_formula=true. Writing extrapolation levels as "measured" is exaggeration, and this paper strictly marks them.

### 9.2 External validity: CPU prototypes are not LLMs, not real machines

UDOS is all Linux CPU deterministic prototypes: the semantic judge is rules not a VLM, the environment is a point-mass double-integrator, tokens are proxy accounting, I/O is asyncio.sleep. Table 2's six-order-of-magnitude difference is deterministic mathematics (shape credible), but translating into real distributed systems' latency and success rate is Chapter 7's GPU/real-machine gate; P7's 100% success is a pilot point estimate under a synthetic environment, not constituting a SOTA claim about real VLA.

### 9.3 Construct validity: "structure" must be reducible to T/G/M, otherwise it degenerates into an all-purpose explanation

Section 6.5 already self-guarded: if any performance difference is post-hoc attributed to "structure," the central claim becomes unfalsifiable. This section reiterates the defense: each item of the triple $S=(T,G,M)$ has operationalizable measurements (fan-in/pose/module interfaces), all four predictions have refutation conditions, and each row of the master table has figures and evidence levels. **When a "structure difference" cannot be reduced to any item of T/G/M nor give refutation conditions, it is an explanation that should be rejected.**

### 9.4 Statistical validity: single-point small samples must not be written as settled conclusions

None of the four core measurements has error bars/confidence intervals; P6's skill=−0.508 is measured on a synthetically injected defect structure (n=160), and real-world "structure missing" is more complex. This paper does not write any difference as a statistically significant conclusion; all differences are marked "pilot point estimates."

### 9.5 Causal validity: structure is both cause and effect

Hebb and the lottery hypothesis point out structure can be shaped by learning; this paper's controlled comparisons measure "initial structure" effects, not "structure evolving with scale during training." The complete closed loop requires bringing learning dynamics $D$ into structure-evolution tracking—already listed as a Chapter 7 open problem.

### 9.6 Validity-threat master table

| Threat type | Most likely attack | How this paper defends | Unresolved ledger |
|---|---|---|---|
| Internal | Ablations measure fan-in not success rate | Deterministic-mathematics marking, no latency claim | GPU end-to-end comparison |
| External | cpu-proto ≠ real machine | Four-level evidence, no SOTA claim | Real-machine gate |
| Construct | "Structure" becomes all-purpose explanation | Triple operationalization + refutation conditions | —— |
| Statistical | Single seed no error bars | Pilot-point-estimate marking | ≥30 seeds/95% CI |
| Causal | Structure shaped by learning | Controlled-comparison separation | In-training structure-evolution tracking |

---

## 10 Conclusion

This paper argued a restrained, falsifiable central proposition: given fixed components and energy budget, a system's intelligent behavior is primarily determined by its spatial relational structure $S=(T,G,M)$, and structure is the invariant across substrates.

The natural-history road (Chapter 3) shows: from chemical-bond isomerism and carbon-chain chirality flips, to protein folding and denaturation, to neuronal connectomes and grid cells, four scales repeatedly stage the same causal chain—**components unchanged, only structure changes, and function changes.** The engineering road (Chapter 4), through UDOS measurements, made this causal chain a measurable quantity: star vs layered topology pressed coordination overhead by six orders of magnitude, multi-view pose raised novel-view coverage to 0.9957, layered hybrid control with 4.7% takeover reached 100% success, and when structure was missing adding parameters was to no avail (skill=−0.508).

This paper did not overstate. Chapter 6 fully acknowledges: structure is not a sufficient condition, dead brains and denatured proteins remind us structure × dynamics × energy are all indispensable; random structure also works; learning itself shapes structure; this repository's CPU prototypes have limited external validity. This paper's boundaries are strictly constrained by the four predictions' refutation conditions—they are this paper's epitaph.

Closing to the title: **spatial structure is itself intelligence.** This sentence is no flattering rhetoric but an engineering instruction: measure structure, design structure, treat structure as the fourth axis alongside parameters, data, and compute. When a system shows intelligence, do not rush to praise its material or its training volume—first ask what its spatial relations look like.

---

## References

> Note: the following AI/systems literature is reused from this volume's *Master Reference Library* (verification date 2026-09-19, status ✅), and this paper does not rewrite the library itself; newly added natural-science classics were verified one by one on 2026-09-19 using general_search/web, and those that could not be verified are marked [CITATION NEEDED], strictly forbidding fabricated DOI/years.

### A. Natural-science classics (newly added in this paper, verified 2026-09-19)

1. **Watson, J. D. & Crick, F. H. C.** (1953). *Molecular Structure of Nucleic Acids: A Structure for Deoxyribose Nucleic Acid.* **Nature**, 171(4356), 737–738. (Double helix and complementary base pairing; 1962 Nobel Prize in Physiology or Medicine, shared with Wilkins; Rosalind Franklin and Gosling's X-ray diffraction was key evidence.) Verification sources: NIH Profiles in Science / NHGRI / Nature 171:737–738 bibliographic record. ✅
2. **Anfinsen, C. B.** (1973). *Principles that Govern the Folding of Protein Chains.* **Science**, 181(4096), 223–230. (Anfinsen principle/thermodynamic hypothesis: amino-acid sequence determines tertiary structure; ribonuclease denaturation/renaturation; 1972 Nobel Prize in Chemistry.) PubMed: 4124164. Verification sources: official Nobel Prize page / Science 181:223–230 bibliography. ✅
3. **Pauling, L.** (1939). *The Nature of the Chemical Bond and the Structure of Molecules and Crystals.* Cornell University Press. (Chemical bonds determine molecular structure, molecular structure determines behavior; proposed the α-helix with Corey and Branson in 1951.) Verification sources: Lindau Nobel Mediatheque / Berkeley Graduate Lectures. ✅
4. **Ramón y Cajal, S.** (silver-stain observations from about 1888; 1906 Nobel Prize in Physiology or Medicine, shared with Golgi). *Neuron doctrine*: the nervous system is made of discrete neurons transmitting signals through synaptic contact regions ("neuron" named by Waldeyer 1891, "synapse" by Sherrington). Verification sources: official Nobel Prize 1906 page / Scholarpedia Cajal entry. ✅
5. **Hebb, D. O.** (1949). *The Organization of Behavior: A Neuropsychological Theory.* New York: Wiley. (Co-firing strengthens synaptic efficiency, cell assemblies; "fire together, wire together" is a later mnemonic condensation, not the author's original words.) Verification sources: Frontiers history of synaptic plasticity / multiple neuroscience textbooks retelling the original sentence. ✅
6. **Jumper, J., Evans, R., Pritzel, A. et al.** (2021). *Highly accurate protein structure prediction with AlphaFold.* **Nature**, 596(7873), 583–589. DOI:10.1038/s41586-021-03819-2. (CASP14 median backbone accuracy 0.96 Å r.m.s.d.-95.) Verification sources: EMBL-EBI citation page / Nature 596:583–589. ✅
7. **Tolman, E. C.** (1948). *Cognitive maps in rats and men.* **Psychological Review**, 55(4), 189–208. DOI:10.1037/h0061626. (Cognitive map: the rat's internal representation of the environment's spatial structure, not a stimulus-response chain.) Verification sources: Psychological Review 55(4):189–208 bibliography. ✅
8. **O'Keefe, J.** (1971). Discovery of hippocampal place cells; **Moser, M.-B. & Moser, E. I.** (2005). Discovery of entorhinal grid cells. They shared the **2014 Nobel Prize in Physiology or Medicine** with O'Keefe. Verification source: official Nobel Prize 2014 press release. ✅ (Note: this paper did not open the precise journal volume/pages of O'Keefe's 1971 original paper and the Mosers' 2005 Nature paper one by one; when precise citation is needed in writing, use the official Nobel page.)
9. Isomerism/structural isomerism: same molecular formula, different atomic connection order, different physical/chemical properties (e.g. n-butane vs isobutane; ethanol vs dimethyl ether $\mathrm{C_2H_6O}$). Verification sources: organic-chemistry textbooks/encyclopedias general statements. ✅ (Textbook-level common knowledge, no single original source needed.)

### B. AI and systems literature (reused from *Master Reference Library*, 2026-09-19 ✅)

10. **Vaswani, A. et al.** (2017). *Attention Is All You Need.* NeurIPS 2017. arXiv:1706.03762.
11. **Hoffmann, J. et al.** (2022). *Training Compute-Optimal Large Language Models (Chinchilla).* NeurIPS 2022. arXiv:2203.15556.
12. **Kaplan, J. et al.** (2020). *Scaling Laws for Neural Language Models.* arXiv:2001.08361.
13. **Fedus, E., Zoph, B. & Shazeer, N.** (2021). *Switch Transformer.* JMLR 2021. arXiv:2101.03961.
14. **Castro, M. & Liskov, B.** (1999). *Practical Byzantine Fault Tolerance (PBFT).* OSDI 1999.
15. **Smith, R. G.** (1980). *The Contract Net Protocol.* IEEE Trans. Computers, C-29(12). DOI:10.1109/TC.1980.1675516.
16. **Dorigo, M., Maniezzo, V. & Colorni, A.** (1996). *The Ant System.* IEEE Trans. SMC-B, 26(1).
17. **Amdahl, G. M.** (1967). *Validity of the Single Processor Approach to Achieving Large Scale Computing Capabilities.* AFIPS SJCC 1967.
18. **Ahn, M. et al.** (2022). *Do As I Can, Not As I Say (SayCan).* arXiv:2204.01691.
19. **Mildenhall, B. et al.** (2020). *NeRF: Neural Radiance Fields.* ECCV 2020. arXiv:2003.08934.
20. **Kerbl, B. et al.** (2023). *3D Gaussian Splatting.* SIGGRAPH 2023. arXiv:2308.04079.
21. **Curless, B. & Levoy, M.** (1996). *A Volumetric Method for Building Complex Models from Range Images (TSDF).* SIGGRAPH 1996. DOI:10.1145/237170.237269.
22. **Krogh, A. & Vedelsby, J.** (1995). *Neural Network Ensembles, Cross Validation, and Active Learning.* NeurIPS 1995.
23. **Yin, M. et al.** (2019). *HotStuff.* PODC 2019. arXiv:1803.05069.
24. **Wu, Q. et al.** (2023). *AutoGen: Multi-Agent Conversation Framework.* arXiv:2308.08155.

### C. Unverified placeholders (forbidden to use as real references)

- `[CITATION NEEDED]`: precise authors/years of the original reservoir-computing (echo-state network) literature and the original lottery-ticket-hypothesis literature; precise journal volume/pages of O'Keefe 1971 and the Mosers 2005 original papers—this paper only cites their Nobel conclusions, with precise sources pending retrieval verification.

---

## Appendix A: UDOS measurement reproduction index

> All figures trace to release-v5.4.4/udos-engine/reports7/ and udos7/topology/. Reproduction commands, seeds, and evidence levels follow the appendices of the topical papers (P1–P12).

| Figure cited in this paper | Source file | Field | Evidence level |
|---|---|---|---|
| Star fan-in 20,971,520 / rounds 8,388,608; layered fan-in 9 / rounds 14 | matrix_scale_demo.json | clean.scale[*] | cpu-proto |
| Multi-view IoU 0.7147→0.8897, coverage 0.7456→0.9957 | spatial_novelview_demo.json | results.single_view/multi_view | cpu-proto |
| 3DGS prototype IoU 0.7285; Sim2Real 1.0/0.5 | splat_transfer_demo.json | gaussian_splat/sim_to_real | cpu-proto |
| Hybrid 100%, takeover 4.7% | P7 abstract + embodied_hybrid_demo.json | aggregate | cpu-proto pilot point estimate |
| Acceleration skill=−0.508 | v7_verification.json | estimator_param_error.accel_a | cpu-proto (main gate verified) |
| mse=a+b/k, a=0.009252, b=−4.23e-5 | agent_scaling.json | quality_saturation_fit | k≤2 verified / k>2 cpu-proto |
| Equal-weight bad-expert MSE 0.2131 | agent_scaling.json | quality_curve[*].naive_equal_2_mse | verified |
| State-coverage oracle 0.695→0.951 | egodata_flywheel_demo.json | passive/coverage_aware_collection | cpu-proto |
| Stigmergy 60 vs contract-net 240 messages | P1 paper + udos7/topology/stigmergy.py | contract_net_message_count | cpu-proto |
| Market budget conservation 240 | matrix_scale_demo.json | clean.market.conserved | cpu-proto |
| World model 967,796 parameters, blind overall 0.01037 | v7_verification.json | params/test_metrics.blind | verified |

---

*(End of full text. This paper is the UDOS Reasoning Engine v7.5.0 whole-volume flagship synthesis paper P0, mutually verified with the P1–P12 topical papers; detailed reproduction steps for all UDOS figures are in the topical papers' appendices.)*

---

> **Trilogy interlinks**: this paper is the "structure axis." The prologue is P0c *Metacognition: To Know What One Knows and What One Does Not Know — Engineering AI Self-Cognition and Boundary Calibration* (self-boundary axis); the scale axis is P0b *Structure + Scale, Everything Follows Scaling Laws*.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Structure Plus Scale Is the Intelligence: Everything Has Scaling Laws

**Working Title (EN):** *Structure Plus Scale Is the Intelligence: A Three-Section Scaling Law Across Substrates*

> Flagship companion paper P0b · Sequel to P0 "Spatial Structure Is the Intelligence" · UDOS Reasoning Engine v7.5.0 · 2026-09-20
>
> **In one sentence**: P0 argues that "structure determines intelligence," and this paper adds the other half — "scale." Structure determines what the growth curve looks like, and scale determines which section of the curve you stand on; **adding scale on a wrong structure does not make you stronger — it makes you weaker.**
>
> **Evidence scope (stated only once in the whole text)**: the figures produced by the UDOS engine itself all come from `reports7/*.json`, run on an ordinary CPU with fixed random seeds and reproducible. They fall into three grades: `verified` (guarded by automated tests), `cpu-proto` (trends seen on a CPU prototype, whose constants have not been calibrated across multiple seeds), and `unverified` (external reports or visions, used only as motivation). Every single-seed pilot is explicitly labeled a "pilot," and ≥30 seeds are required for recomputation before submission. External literature was verified item by item on 2026-09-20.

---

## Abstract

Over the past decade, the success of artificial intelligence can be summed up in almost one word: go big. Larger models, more data, and ample compute — and loss falls along a beautiful power-law curve. But this "bigger is better" narrative secretly assumes one thing: **the structure of the network is fixed** — everyone uses the Transformer, stacks layers in the established way, and only adds scale along this fixed track.

This paper puts the frozen "structure" back in and proposes a proposition that can be killed by experiment. We write capability as a function of two variables: structure $S$ (how things are connected and partitioned) and scale (parameters/nodes $N$, data $D$, compute $C$). Across seven domains — biological metabolism, cities, brains, chip interconnects, and large models — the growth of capability with scale all exhibits the same **three-section curve**: first a steady power-law growth, then a structural phase transition at some critical point, and finally saturation or even a downward turn. **The shape of the curve is determined by structure, and which section of the curve the system stands on is determined by scale.**

On our own multi-agent engine, we saw the prototype of this curve through seven controlled comparisons: with the same one million nodes, the center of a star topology must simultaneously process about 20.97 million messages with a critical path as long as 8.39 million rounds, while in a hierarchical topology each node faces at most 9 messages and finishes in 14 rounds — this is not "a bit more efficiency," but the growth exponent changing from 1 to 0; increasing concurrent agents from 1 to 2 raises throughput by 1.59×, but continuing to 8 is slower than 1; equal-weight insertion of an unqualified member into the team makes error surge from 0.0092 to 0.2131, while simply increasing the number of qualified members barely moves the error.

This paper does not claim to have found a theory of everything. The harshest objections — "the emergence turning point may be an illusion created by the measurement method" and "a single-seed pilot is not sufficient evidence" — are listed item by item in Chapter 7, along with what results would falsify this paper.

**Keywords**: scaling law; structure and scale; three-section curve; phase-transition turning point; saturation and negative return; falsifiable prediction; UDOS; multi-agent

---

## 1 Starting from Three Simple Phenomena

### 1.1 Why an elephant is not a thousand mice

An elephant's body mass is about a hundred thousand times that of a mouse, but its metabolic rate is not a hundred thousand times — it is roughly the three-quarter power of a hundred thousand times; when body mass doubles, the energy needed to sustain life increases by only about 75%. This is the pattern the biologist Kleiber discovered in 1932, later called Kleiber's law.

A larger body is not free. Blood vessels must branch again and again to deliver nutrients to every cell; the longer the network, the greater the "structural discount" of transport. **The returns from scale growth are inherently discounted, and the discount rate is determined by the structure of the transport network.**

### 1.2 Larger cities are more economical, and also more profitable

In 2007 the physicist Bettencourt and colleagues analyzed a large amount of urban data and found a symmetric phenomenon: when urban population doubles, infrastructure such as roads, cables, and gas stations increases by only about 80% (sublinear, economical), while innovation outputs such as patents, wages, and GDP increase by about 120% (superlinear, profitable).

On the same "population scale" axis, different kinds of "structural quantities" give oppositely directed exponents. This shows that scale itself has no fixed good or bad; **good or bad depends on what you are counting and how those things are organized.**

### 1.3 Two agents are the fastest, eight are the slowest

On our own engine, an even more counter-intuitive phenomenon is directly reproducible. Let concurrent agents process a batch of mutually independent tasks:

- With 1 agent, 153 tasks per second;
- With 2, a peak of 244 (1.59× speedup);
- With 4, it falls back to 158;
- With 8, only 106 remain — **slower than 1.**

But as soon as the task type is changed from "competing for CPU" to "waiting for external input" (e.g., waiting on the network or disk), increasing from 1 to 8 makes throughput grow almost perfectly linearly to nearly 8×.

The scale axis is identical, and the only thing changed is "whether the task structure and the hardware structure match." When they match, adding scale is nearly free; when mismatched, adding scale is making a mess of the system.

These three phenomena — metabolism, cities, and agents — together point to the central thesis of this paper:

> **Scale is never a good thing, nor a bad thing. The returns of scale are a function of structure.**

---

## 2 Making It Clear: Two Variables, One Curve

### 2.1 What structure is, what scale is

Following the definition of companion paper P0, **structure** includes three things:

- **Connection topology**: who connects to whom and in which direction information flows (corresponding to star/hierarchical/mesh in multi-agent systems);
- **Spatial organization**: how connections are geometrically arranged (distance, viewpoint, fan-in/fan-out);
- **Module boundaries**: how the system is layered, encapsulated, and interfaced between modules.

**Scale** is decomposed into three measurable axes:

- **$N$ (nodes/parameters)**: how many agents, how many parameters, how many neurons;
- **$D$ (data/experience)**: how many trajectories and tokens have been seen;
- **$C$ (compute/energy)**: how many FLOPs, how much power, how much wall-clock time.

Thus capability can be written as a binary function:

$$\text{Capability} \approx F\big(\text{structure } S;\; N,\, D,\, C\big) \tag{1}$$

This formula can be understood without a mathematical background: **capability depends simultaneously on "how it is organized" and "how large it is organized."**

### 2.2 The three claims of this paper

**Claim one: structure determines the shape of the curve.** On log-log coordinates, how capability grows with scale, where it bends, and where it peaks are all prescribed by structure. Changing the connection mode does not raise the curve a little; it replaces it with a different curve.

**Claim two: scale determines which section of the curve you stand on.** Once structure is fixed, adding scale only moves along the established curve; it cannot change the curve's shape.

**Claim three: when structure is mismatched, the marginal return of scale is negative.** When the mode of organization does not match the task or resources, continuing to add people, parameters, or machines makes capability fall rather than rise. Section 1.3's "8 agents are slower than 1" is a direct observation of this claim.

As an analogy: **structure is grammar, scale is length.** With wrong grammar, the longer the writing the more incoherent it becomes; with right grammar, length begins to produce value.

### 2.3 The three-section curve: turning "everything has" into a falsifiable proposition

If "everything has a scaling law" means only that "any data can be fitted to a power law," it is an unfalsifiable empty phrase. We tighten it into a far more concrete proposition — the **three-section curve**:

![Figure 4: Three-section master curve, schematic](figures/P0b_fig4_master_curve.png)

> Figure 4: A three-section schematic of capability versus scale (a schematic, not measured data). Section 1 is steady power-law growth; Section 2 shows a turning point at the structural phase transition; Section 3 saturates or turns downward. A good structure pushes the turning point further right; a wrong structure makes it arrive early.

- **Section 1 · Local power-law region**: structure is correct and scale is still small; capability grows steadily with scale, approximately a straight line on log-log coordinates;
- **Section 2 · Structural phase-transition turning point**: scale reaches the threshold where the old structure cannot hold, and the system must change its mode of organization (from one center to multi-layer division of labor, from a single expert to routed experts); the curve bends here;
- **Section 3 · Saturation or negative-return region**: after hitting the communication, data, or energy wall, growth flattens or even turns downward.

This proposition can be killed: if, in a controlled experiment with structure strictly fixed, capability keeps growing linearly along the same slope across more than tenfold scale without peaking or bending, the three-section curve is wrong.

---

## 3 Nature Has Walked This Path for Three Billion Years

Before discussing our own experiments, let us see whether different domains repeatedly exhibit the same curve. Table 1 is the overview, followed by the most persuasive layers.

**Table 1 Overview of scaling phenomena across seven domains (exponents are empirical ranges, not universal constants)**

| Domain | Structural unit | Scale axis | Observed pattern | Turning point/wall |
|---|---|---|---|---|
| Chemical bonds | Bond type and crystal lattice | Atoms/unit cells | Properties jump with bond type, not smoothly with atom count | Bond-type switch is a phase transition |
| Proteins | Sequence → folding | Residues | Small proteins have one domain; large proteins are modular with multiple domains | Heat denaturation, function falls off a cliff to zero |
| Life metabolism | Fractal transport vessel network | Body mass | Metabolic rate ∝ body mass to the 3/4 power (Kleiber's law) | Surface-area/volume wall of cell size |
| Chips/compute | Interconnect topology | Cores/chip count | Amdahl's law caps it; USL predicts adding machines makes it slower | Communication wall, power wall |
| Battery cells | Series-parallel and battery management | Cell count | Voltage with series, capacity with parallel are approximately linear | Thermal-runaway threshold (lacks rigorous measurement, left blank) |
| Brain | Connectome and cortex | Neuron count | The human brain has about 86 billion neurons, scaling linearly by primate rules | White-matter wiring cost, cortical folding |
| Large models | Attention/routing structure | Parameters/data/compute | Loss falls as a power law with parameters (exponent about 0.076) | "Emergence" turning point (may be a measurement illusion) |

### 3.1 Metabolism: the 3/4 discount rate is computed from vessel structure

The deepest point of Kleiber's law is not the figure itself but its origin. In 1997, West, Brown, and Enquist gave a structural explanation in *Science*: blood vessels are a fractal tree branching from the aorta all the way to capillaries; to keep terminal capillary size independent of body size (because cell size is universal) while minimizing transport energy, the geometry of this tree must satisfy specific self-similar relationships — **the 3/4 power was not assumed by anyone; it is derived from the geometric constraints of the transport network.**

Replace it with a non-branching straight tube and the exponent is no longer 3/4. This is precisely the natural template for claim one: **the scaling exponent is a product of structure, not an accident of scale.**

It must be honestly stated that 3/4 is an empirical approximation rather than an iron law; some scholars point out that it is not statistically significantly better than the naive 2/3, and the exponent drifts between 2/3 and 1 across species and body temperatures. We cite it not to carry 3/4 as a universal constant into AI, but to use it to prove that "deriving the growth exponent from structural geometry" is possible.

### 3.2 Proteins: denaturation is a phase transition with no intermediate state

A protein's function depends entirely on its three-dimensional folded shape, and folding is a cooperative process: under slow heating nothing happens, but at some narrow temperature interval the hydrogen bonds maintaining the shape collectively break, and the protein jumps in an instant from the native state to the denatured state, with catalytic function falling off a cliff to zero — there is no "half-inactivated" intermediate state.

This is the cleanest natural sample of the "structural phase-transition turning point" in the three sections: **structure is effective on one side of the turning point and fails on the other.** Moreover, small proteins fold as one whole block, while large proteins are split into multiple independently folded domains linked together — larger scale forces the system to phase-transition from "one whole block" to "modular assembly," the same principle as replacing a single feed-forward network with multiple routed experts (MoE) in large models.

### 3.3 Brains: long-distance wiring grows faster than computational units

"The human brain has a hundred billion neurons" is a rough estimate that has circulated for half a century. The neuroscientist Herculano-Houzel used an isotropic fractionator to recount, giving about 86 billion neurons, and found that the human brain is not some extraordinary organ but a **primate brain linearly scaled up by standard primate rules**.

More interesting is the allometric growth of white matter versus gray matter: as the brain enlarges, the white matter responsible for long-distance connections grows faster than the gray matter responsible for computation. Wiring cost hits the wall first, and the brain's response is to fold the cortex into wrinkles, shortening connections within a limited skull. **When long-distance communication becomes expensive with scale, the system adds layers and localizes communication** — the same structural move as chips shifting from shared buses to fat-tree hierarchical interconnect, and our replacing a star center with hierarchical topology in the engine.

### 3.4 Chips: "adding machines makes it slower" has a formula

Engineering has long had mature theories of scale returns turning negative.

- **Amdahl's law (Amdahl, 1967)**: as long as a small fraction of a program cannot be parallelized, no matter how many cores are added the speedup is sealed by that fraction — it explains "peaking."
- **The Universal Scalability Law USL (Gunther, 1993)** goes further, predicting the "downward turn":

$$C_p=\frac{p}{1+\sigma(p-1)+\kappa\,p(p-1)} \tag{2}$$

Where $p$ is the number of parallel workers, $\sigma$ is the cost of serialization and resource contention, and $\kappa$ is the cost of keeping state consistent among multiple workers, the latter growing **quadratically** with the number of people. When $\kappa>0$, throughput must monotonically decline after rising to a peak — **the more people, the slower the team.**

Chip history also has an energy wall. The 1974 Dennard scaling once promised "smaller transistors, faster speed, constant power density," driving clock frequency from megahertz to gigahertz; but around 2005–2007, leakage and threshold voltage could no longer shrink proportionally, power density began to spiral out of control, the "free lunch" ended, and the industry was forced to abandon single-core frequency increases for multi-core parallelism (Intel canceling the Tejas project in 2004 was the landmark event). **This itself is a structural phase transition: not that transistors became smaller and slower, but that the path of "continuing single-core frequency stacking" physically hit the wall and structure had to change.**

### 3.5 Large models: AI is the most thoroughly studied domain of scaling laws

Three works form the backbone on the AI side:

- **Kaplan et al. (2020)**: loss falls approximately as a power law with parameters, data, and compute, with an exponent on parameters of about 0.076 — direct evidence of the Section 1 power-law region;
- **Chinchilla (Hoffmann et al., 2022)**: under fixed compute, parameters and data should grow in the same proportion, and stacking parameters without feeding data is wasteful — this is in fact "mix-ratio structure determines efficiency";
- **Switch Transformer (Fedus et al., 2021)**: replace the feed-forward layer with a set of experts, activating only one per forward pass, reaching about 1.6 trillion total parameters with very small per-pass compute. It proves that **routing structure itself is a new scale axis**: not by growing larger, but by changing "which parameters participate in computation."

DeepSeek's NSA sparse attention (2025) is a more recent case of the same kind: without adding parameters, only changing the connection structure of attention (three branches — compression, selection, sliding window — plus gating), it reduces the decoding latency of 64k long context to about one-eleventh. **What truly changes the scale curve is often a structural switch, not simply piling on materials.**

As for "emergence" — certain capabilities being zero on small models and suddenly appearing at some scale — we must immediately introduce the harshest counter-argument: Schaeffer et al. (2023) prove that many such turning points are **illusions selected by the measurement method**: replacing a non-continuous metric such as "whether 50% accuracy is crossed" with a continuous metric, the capabilities of the same batch of models actually grow smoothly and "emergence" disappears. This criticism determines the restrained attitude of this paper: a turning point can only be called a "candidate turning point" until it is reproduced on deterministic metrics across multiple domains.

### 3.6 An honest blank

Of the seven layers, "battery cells" is the thinnest in evidence. We only have circuit common sense such as voltage growing linearly with the number of series cells and capacity growing linearly with the number of parallel cells, plus the engineering experience that "series amplifies individual inconsistency and requires a battery management system for balancing"; we do not have a peer-reviewed rigorous scaling exponent between battery-pack scale and performance.

Under the discipline of this paper, **when evidence is insufficient, leave it blank and mark it `[CITATION NEEDED]`, rather than hard-coding a pretty figure to complete the "seven layers."** This blank itself is a qualification of the words "everything has": it is a program to be tested, not a fact list already filled in.

---

## 4 What We Saw on Our Own Engine

This chapter is the original evidence of this paper. All seven experiments follow the same discipline: **change only one of structure or scale at a time, freezing everything else.** All figures are reproducible on a CPU, with sources in the data-provenance table at the end.

### 4.1 Evidence one: change the connection mode and the growth exponent changes from 1 to 0

Let a star topology (all agents report to one center) and a hierarchical topology (tree-shaped division of labor) handle the same coordination task, changing only the topology, extrapolating from 10 nodes all the way to one million:

**Table 2 Star vs hierarchical: changing only connection structure (first two rows measured, last four closed-form extrapolation, grade cpu-proto)**

| Target nodes | Actually generated nodes | Messages the star center faces | Hierarchical max fan-in | Star critical path (rounds) | Hierarchical critical path (rounds) |
|---|---|---|---|---|---|
| 10 | 73 | 640 | 9 | 256 | 4 |
| 100 | 585 | 5,120 | 9 | 2,048 | 6 |
| 1,000 | 4,681 | 40,960 | 9 | 16,384 | 8 |
| 10,000 | 37,449 | 327,680 | 9 | 131,072 | 10 |
| 100,000 | 299,593 | 2,621,440 | 9 | 1,048,576 | 12 |
| 1,000,000 | 2,396,745 | **20,971,520** | **9** | **8,388,608** | **14** |

![Figure 2: UDOS fan-in/rounds log-log](figures/P0b_fig2_topology_fanin_rounds.png)

> Figure 2: The horizontal axis is node count (log scale). The star topology's center load explodes linearly with scale (exponent 1), the hierarchical topology's per-node load is constantly 9 (exponent 0), and the critical path grows only slowly with the logarithm.

**How to read this table**: at a million nodes the star center must simultaneously process about 20.97 million messages and walk 8.39 million rounds — this is not "a bit more expensive," it is physically impossible; the hierarchical topology's nodes forever face only 9 neighbors and finish in 14 rounds. The difference between the two is not an efficiency constant but a growth exponent: one explodes linearly with scale, one is constantly constant. **This is the most direct reading of "structure determines the curve shape."**

### 4.2 Evidence two: on a mismatched structure, adding people makes it progressively slower

The throughput hump of Section 1.3 is worth fixing with a table. The same batch of agents, the same dual-core machine, changing only the task structure:

**Table 3 Throughput versus concurrency (single-seed pilot, grade cpu-proto)**

| Concurrent agents | CPU-intensive (tasks/s) | Speedup | I/O-waiting (tasks/s) | Speedup |
|---|---|---|---|---|
| 1 | 153.208 | 1.000 | 19.94 | 1.00 |
| 2 | **244.172** | **1.594** | 39.88 | 2.00 |
| 4 | 158.386 | 1.034 | 79.69 | 4.00 |
| 8 | 106.485 | 0.695 | 159.26 | 7.99 |

![Figure 3: Throughput hump](figures/P0b_fig3_throughput_hump.png)

> Figure 3: The CPU-intensive task peaks at 2 agents and then turns, with 8 slower than 1; the I/O-waiting task is nearly perfectly linear. The scale axis is the same, and the curve shape is entirely determined by the structural match between task and hardware.

This is precisely the shape of the USL formula (Eq. 2) under two different $\kappa$ values: when CPU-intensive, agents beyond the number of physical cores contend for compute resources and repeatedly synchronize shared state, with the consistency cost surging quadratically in the number of people and the curve peaking then falling; when I/O-waiting, agents spend their time waiting without contending, and both $\sigma$ and $\kappa$ are near zero, making the curve approximately a straight line.

It must be emphasized that this is a **single-seed, four discrete-level** pilot. "Peak at 2" may partly be an apparent turning point caused by the coarse 2→4 level (exactly the measurement illusion Schaeffer warns of), and whether "8 slower than 1" is stable requires denser levels and ≥30-seed recomputation. We list it as candidate evidence, not a settled conclusion.

### 4.3 Evidence three: a 23× structural difference, only a 0.5% quantity difference

Integrating the judgments of multiple agents, how does quality change with team size? We fit $mse(k)=a+b/k$, obtaining a saturation floor $a=0.009252$, $b=-4.233\times10^{-5}$ ($k\le2$ is verified, larger $k$ is cpu-proto); the error of the single best member is 0.009196, almost coincident with the saturation floor — the ensemble already peaks near $k=1$.

![Figure 5: Quality saturation and model-size convergence](figures/P0b_fig5_saturation_modelsize.png)

> Figure 5: Left, the weighted-ensemble quality of qualified members already hugs the saturation floor near $k=1$, with continued additions giving almost no improvement; wrongly equal-weight inclusion of one unqualified member makes the error jump immediately to 0.2131. Right, the model-width scan selects 967,796 parameters by the "stop when good enough" rule.

The two figures placed together make the conclusion very plain:

- Expanding the team from 1 qualified member to 17 moves the error only about 0.5% around 0.0092 — **the marginal return of quantity is nearly zero**;
- But as soon as one unqualified member is inserted **with equal weight** into the decision, the error jumps immediately from 0.0092 to 0.2131 — **the influence of structure (how to weight, who gets to speak) is about 23× that of quantity.**

When the system already stands in Section 3 (the saturation region), "adding people" is the most ineffective action; "changing structure" is the one. This also explains a common misconception in the multi-agent field: a "legion" piled together by headcount is often inferior to one group with clear division of labor.

### 4.4 Evidence four: with the same data budget, changing the collection method yields 37% more effective data

The scale axis is not only "how much data is used," but also "how data is collected." Under an identical collection budget (48 collection rounds), compare two collection structures: passive volume piling versus active coverage-aware blind-spot scanning:

**Table 4 Active versus passive collection (same budget, grade cpu-proto)**

| Collection structure | Data yield | Effective state units covered | Share of ideal coverage | Redundancy rate | Experience density (units/min) |
|---|---|---|---|---|---|
| Passive collection | 0.667 | 157 | 0.695 | 0.599 | 7.80 |
| Active blind-spot scanning | **0.854** | **215** | **0.951** | **0.692** | **11.38** |

Without adding a cent to the budget, changing only the structure of "what to collect" raises yield from 0.667 to 0.854 and the effective state units collected per minute from 7.80 to 11.38. **The effective slope of the data scale axis is determined by collection structure.** If the 10-millionth hour is merely a low-quality repetition of the first 1 million hours, what it brings is not growth but a storage bill.

The gain from active scanning must rule out a trivial explanation — "it just greedily picked easy samples." A controlled experiment restricting the Agent to greedy selection within the same biased data pool reaches only a yield of 0.7917 and coverage share 0.889; while active scanning that can schedule new collection and truly fill blind spots reaches 0.854 / 0.951. **The gain comes mainly from "collecting where one has not been," not from "selecting within old data."**

### 4.5 Evidence five: models are not the bigger the better, but "stop when good enough"

Scanning the hidden-layer width of the world model (64/128/256), performance improves monotonically and parameters grow from 95,000 to 968,000:

**Table 5 Model-size scan (grade verified)**

| Hidden width | Parameters | Validation metric | Blind-test error |
|---|---|---|---|
| 64 | 95,348 | 0.01190 | 0.01527 |
| 128 | 271,476 | 0.00950 | 0.01172 |
| 256 | **967,796 (selected)** | 0.00821 | 0.01037 |

The selection rule is "take the **smallest** model whose metric is within a 3% tolerance of the optimal value." In this scan performance was still improving, so the largest level was selected; but the rule itself embodies the Chinchilla spirit — **scale should match the task, rather than mindlessly taking the largest.** Whether saturation or even degradation appears at wider widths (512/1024) is not supported by this data and is marked `[RESULT NEEDED]`.

### 4.6 Evidence six: governance also has a scale tax, and structure can bargain it down

As scale grows, cost lies not only in compute but in coordination and trust. Three small experiments:

- **Byzantine fault tolerance**: to tolerate $f$ malicious nodes, the committee size must satisfy $n\ge 3f+1$ with a $2f+1$ quorum — security carries a scale tax; under $n=7,f=2$, all 24 tasks were adjudicated correctly;
- **Internal market accounting**: budget, payment, and balance are strictly conserved (240 = 240 = 240); conservation is a cross-scale invariant, and mismatched accounts indicate something wrong with system state;
- **Environmental-medium collaboration (stigmergy)**: with the same 30 tasks and 6 agents, collaborating by leaving traces on one shared blackboard needs only 60 messages, while peer-to-peer negotiation one by one needs 240 — **changing the communication structure from "pairwise dialogue" to "environmental note-taking" reduces communication volume to a quarter.**

### 4.7 Evidence seven: spatial structure also saturates

Predicting a scene from an unfamiliar viewpoint, using only 1 training viewpoint, the contour overlap IoU is 0.7147, spatial coverage 0.7456, precision 0.9472, depth error MAE 0.3186 (voxel resolution 0.1 m, 6 held-out viewpoints); using 8 pose-tagged viewpoints raises IoU to 0.8897 and coverage to 0.9957. But split by viewpoint, **the first two viewpoints contribute the vast majority of the gain, and later viewpoints' marginal contribution rapidly shrinks** — once the viewpoint structure is correct, a few viewpoints approach saturation, and continuing to pile on viewpoints yields little.

### 4.8 The shape assembled from seven pieces of evidence

**Table 6 Overview of seven pieces of evidence**

| # | Experiment | Changed structure | Scale axis | Key figures | Grade |
|---|---|---|---|---|---|
| 1 | Star vs hierarchical | Topology | Node count | Center load 20.97 million → 9; rounds 8.39 million → 14 | cpu-proto |
| 2 | Throughput hump | Task structure | Concurrency | 2 peak, 8 slower than 1 | cpu-proto |
| 3 | Quality saturation | Weighted/equal | Member count | 23× structural difference, 0.5% quantity difference | verified/cpu-proto |
| 4 | Data flywheel | Collection structure | Data budget | Yield 0.667→0.854 | cpu-proto |
| 5 | Model selection | Width | Parameters | "Stop when good enough" selects 968,000 parameters | verified |
| 6 | Governance scale tax | Communication/trust structure | Node count | Messages 240→60; $n\ge3f+1$ | cpu-proto |
| 7 | Multi-view | Spatial structure | Viewpoint count | IoU 0.7147→0.8897, diminishing gains | cpu-proto |

Putting them back into the three sections: the hierarchical side of evidence 4 and evidence 1 falls in Section 1 (correct structure, small scale, steady growth); the star→hierarchical switch of evidence 1 and the negotiation→blackboard switch of evidence 6 fall in Section 2 (structural phase transition pushes the turning point open); the hump of evidence 2, the saturation of evidence 3, and the marginal decline of evidence 7 fall in Section 3. **The seven fragments assemble the prototype of the three-section curve on silicon agents — only a prototype.**

---

## 5 How to Prove This Paper Wrong

The value of a scientific proposition lies in what can kill it. From the original five predictions we retain the three hardest, each with its criterion written out.

**Prediction one (reproducible three sections)**: fix structure, and capability should first grow as a power law with scale, then flatten or turn negative after crossing some wall; and this shape must be reproduced in at least three mutually unrelated domains before it deserves to be called a cross-domain law.
*How to kill it*: in strictly controlled comparisons, scale extends linearly along the same slope by more than tenfold without peaking or bending — the three sections degrade to the weak claim of "local power law."

**Prediction two (structural jump under equal parameters)**: under identical parameters, compute, and data, changing only the topology from flat to hierarchical/routed should produce a discontinuous step in capability rather than a smooth gradient.
*How to kill it*: in equal-budget comparisons, the end-to-end performance of the two structures falls in the same confidence interval and is stably reproduced across multiple seeds — that would mean structure affects only an efficiency constant, and this paper exaggerated it.

**Prediction three (mismatch must be negative + cross-domain collapse)**: adding scale under a mismatched structure should stably worsen (after multi-seed recomputation "8 slower than 1" still holds); further, after normalizing the scale axes of each domain by structural characteristic quantities, the seven curves should collapse onto the same family of three-section curves.
*How to kill it*: after ≥30-seed recomputation the "the more added the slower" difference includes zero within the 95% confidence interval; or the seven normalized curves cannot collapse together at all — then this paper honestly retreats to "multi-domain analogy," and "everything has" is downgraded to "each has its own law." This is the most aggressive and most kill-worthy claim of the paper.

---

## 6 What This Means for Building Systems

**First, settle structure before discussing scale.** The order of architectural decisions should be reversed: not "first decide how large a model, then casually build a structure," but first choose the right topology, routing, and module boundaries, then choose scale on the curve prescribed by that structure. Trying to run farther on the wrong curve only hits negative returns faster.

**Second, ask three questions before "adding people and machines."** Will communication surge quadratically with headcount (USL's $\kappa$)? Does the new member bring new information, or a replica of the strongest member (evidence three: replicas produce no value)? Has this scale hit an energy, latency, or trust wall?

**Third, treat the turning point as a friend, not a failure.** The brain uses cortical folding for the wiring wall, the chip uses multiple cores for the power wall, and the model uses MoE routing for the parameter wall — every saturation is a prompt that it is time to change structure. The scalability of a system is, in essence, its ability to "actively switch structure."

**Fourth, remain restrained about inferences toward AGI.** If this paper holds, general intelligence will not be the result of "the same network growing infinitely large," but more likely "a new structure + choosing the right scale on its curve." We do not predict when AGI arrives, but only offer a stance: **treating "finding the right structure" itself as an engineering problem that can be searched and measured is more worth investing in than piling scale on a fixed structure.**

---

## 7 Where We Might Be Wrong

The lifespan of a flagship paper depends on whether it writes the harshest objections up front.

1. **The turning point may be a measurement illusion.** Schaeffer et al. have shown that large-model "emergence" largely comes from the choice of non-continuous metrics. Our defense is: the six-orders-of-magnitude difference in topology fan-in is deterministic mathematics (closed-form formulas consistent with explicit simulation), not subjective scoring; but the throughput hump was indeed measured at only four discrete levels and must be recomputed with denser scales. When a "phase transition" cannot be distinguished from a "measurement artifact," it does not deserve to be called a phase transition.

2. **Some walls are physical and cannot be bypassed by changing structure.** The end of Dennard scaling is a physical result of leakage and threshold voltage, and hierarchical topology can only push the communication wall back, not eliminate it. This paper does not claim structure is omnipotent.

3. **Structure itself is also learned.** The lottery-ticket hypothesis (Frankle & Carbin, 2019) points out that training screens out sparse "winning" substructures from randomly initialized networks; biological neural connections are also shaped by learning. In our comparisons, structure was manually arranged before the experiment began, so they only prove that "**designed** structure affects the scale curve," not that structure "**emerging** during training follows the same law." Filling this gap requires continuously tracking structural metrics such as modularity and path length during training — a clear next step.

4. **Analogies have boundaries.** Metabolic 3/4 is the result of three billion years of evolution and physical transport, while agent systems are products of human design; urban scaling laws themselves have data-scope disputes; Metcalfe's law "network value grows quadratically with node count" has been pointed out to overestimate the value of heterogeneous connections. We only borrow the "shape," not the "constants": 3/4, 0.076, 1.2, and 9 are not the same number and never will be.

5. **All engine figures are only single-seed pilots on a CPU prototype.** The judge is a rule rather than a real vision model, the physical environment is a point-mass proxy, and the I/O curve is simulated with wait time rather than a real interface. They can prove "the shape of the mechanism holds in a simplified world," not "the constants hold in a real system." This paper therefore positions itself as **a research program plus preliminary silicon evidence**, not a proven theory.

---

## 8 Conclusion

P0 says: spatial structure is the intelligence. This paper adds the other half it froze: **structure determines the shape of the growth curve, scale determines which section of the curve the system stands on; adding scale on a wrong structure yields negative returns.**

From metabolism's 3/4 power, to cities' 1.2 and 0.8, to the brain's cortical folding, to the chip's power wall, and to the "2 agents fastest, 8 slowest" hump in our engine, different domains repeatedly exhibit the same three-section curve. But we insist on claiming only "shape isomorphism," not "universal constants" — this far weaker claim is the one that can be taken seriously by experiment.

There is a saying in the field: we are not creating intelligence, we are translating it. In blood vessels, brains, and chemical bonds, nature has already trial-and-erred for billions of years on "how structure and scale work together"; what engineering must do is translate it domain by domain. What this paper offers is not an answer but a set-up target: predictions one, two, and three can kill it at any time, and until killed it deserves serious testing.

---

## References

> AI/systems literature reuses this volume's "Master Reference Library" (verified 2026-09-19); the following entries were verified item by item on 2026-09-19/20, and those that cannot be verified are marked `[CITATION NEEDED]` and are not treated as real citations.

1. West, G. B., Brown, J. H., & Enquist, B. J. (1997). A General Model for the Origin of Allometric Scaling Laws in Biology. *Science*, 276(5309), 122–126. DOI:10.1126/science.276.5309.122.
2. Kleiber, M. (1932). Body size and metabolism. *Hilgardia*, 6, 315–353; and (1961) *The Fire of Life*. Wiley. (Counter: Hulbert et al. point out 3/4 is only an empirical approximation.)
3. Bettencourt, L. M. A. et al. (2007). Growth, innovation, scaling, and the pace of life in cities. *PNAS*, 104(17), 7301–7306. DOI:10.1073/pnas.0610172104.
4. Herculano-Houzel, S. (2012). The remarkable, yet not extraordinary, human brain as a scaled-up primate brain. *PNAS*, 109(Suppl 1), 10661–10668. DOI:10.1073/pnas.1201895109.
5. Amdahl, G. M. (1967). Validity of the Single Processor Approach. *AFIPS SJCC*.
6. Gunther, N. J. (2008). A General Theory of Computational Scalability Based on Rational Functions. arXiv:0808.1431 (USL).
7. Dennard, R. H. et al. (1974). Design of ion-implanted MOSFET's with very small physical dimensions. *IEEE JSSC* (its scaling failed around 2005–2007, and the industry shifted to multi-core).
8. Kaplan, J. et al. (2020). Scaling Laws for Neural Language Models. arXiv:2001.08361.
9. Hoffmann, J. et al. (2022). Training Compute-Optimal Large Language Models (Chinchilla). arXiv:2203.15556.
10. Fedus, E., Zoph, B. & Shazeer, N. (2021). Switch Transformers. arXiv:2101.03961.
11. Liang Wenfeng et al. (2025). Native Sparse Attention. arXiv:2502.11089.
12. Schaeffer, R., Miranda, B. & Koyejo, S. (2023). Are Emergent Abilities of Large Language Models a Mirage? arXiv:2304.15004.
13. Frankle, J. & Carbin, M. (2019). The Lottery Ticket Hypothesis. ICLR. arXiv:1803.03635.
14. Briscoe, B., Odlyzko, A. & Tilly, B. (2006). Metcalfe's Law is Wrong. *IEEE Spectrum*, 43(7), 26–31 (an economic analogy, not a physical law).
15. Dunbar, R. (1990s) Dunbar's number is about 150 (the causal direction is disputed; cite with caution).
16. Vaswani, A. et al. (2017). Attention Is All You Need. arXiv:1706.03762.
17. Smith, R. G. (1980). The Contract Net Protocol. *IEEE Trans. Computers*.

`[CITATION NEEDED]`: a peer-reviewed measured exponent for battery-pack scale; the exact volume/pages of the original Dennard 1974 and Dunbar papers.

---

## Appendix: Provenance Table for the UDOS Figures in This Paper

| Figure | Data file | Grade |
|---|---|---|
| Fan-in 20,971,520→9, rounds 8,388,608→14 (six levels) | `matrix_scale_demo.json` | cpu-proto (first two measured) |
| CPU throughput 153.208/244.172/158.386/106.485; I/O 19.94/39.88/79.69/159.26 near-linear | `agent_scaling.json` | cpu-proto (single seed) |
| Quality saturation $a=0.009252,b=-4.233\times10^{-5}$, best single 0.009196; equal weight 0.2131 | `agent_scaling.json` | $k\le2$ verified, rest cpu-proto |
| Yield 0.667→0.854; state units 157→215; density 7.80→11.38; same-pool greedy ablation 0.7917/0.889 | `egodata_flywheel_demo.json` | cpu-proto (48 rounds/144 minutes, oracle 226 units) |
| Width 64/128/256 = 95,348/271,476/967,796 parameters | `model_size_convergence.json` | verified |
| Stigmergy 60 vs Contract Net 240; $n\ge3f+1$ | `udos7/topology/` | cpu-proto |
| Multi-view single-view IoU 0.7147/coverage 0.7456/precision 0.9472/depth MAE 0.3186; eight-view IoU 0.8897/coverage 0.9957 | `spatial_novelview_demo.json` | cpu-proto (8 training views/6 held out/voxel 0.1 m) |

*(End of full text. P0b is the second axis "scale" of the flagship trilogy; the structure axis is in P0, and the self-boundary axis is in P0c.)*


---

<p align="center"><img src="assets/logo.png" width="180" alt="Twin Earth"/></p>

# Metacognition: Know What You Know and What You Do Not — Engineering AI Self-Knowledge and Boundary Calibration

**Working Title (EN):** *Engineering AI Self-Knowledge: Calibration, Abstention, and Identifiability*

> Prologue P0c of the flagship trilogy · together with P0 (*Space Structure Is Itself Intelligence*) and P0b (*Structure + Scale Is Itself Intelligence*) it forms the three axes "structure × scale × self-boundary" · UDOS reasoning engine v7.5.0 · 2026-09-20
>
> **The thesis in one sentence.** Confucius said: "To know what you know and what you do not know, that is true knowledge." Today's AI fails exactly here — its knowledge is nearly unbounded, yet it does not know where its own answers stop being reliable. This paper does not ask whether AI is conscious. It asks one engineering question: **how do we make a machine honestly say "I don't know" when it doesn't know?**
>
> **Evidence policy (stated once).** Every UDOS number below comes from `reports7/*.json`, produced on an ordinary CPU with fixed random seeds and fully reproducible runs. Levels: `verified` (guarded by automated tests), `cpu-proto`/`cpu-proxy` (trends observed on a CPU prototype), `unverified` (external motivation only). Single-seed pilots are labeled as such; ≥30 seeds are required before submission. References were checked individually on 2026-09-20.

---

## Abstract

The strongest language models today are, at bottom, supercharged probability matchers: given a question, the model finds the statistically most likely continuation in a vast parameter space and renders it in fluent prose. This mechanism has one fatal gap — **it has no concept of "I don't know."** Confronted with a question outside its training data, it does not pause and acknowledge a boundary; it confidently produces a plausible-looking wrong answer. That is the root of hallucination: not a lack of knowledge, but a lack of metacognition — a sense of the boundaries of one's own knowledge.

This paper deliberately sets aside the metaphysical question of machine consciousness. Instead it reduces "self-knowledge" to three quantities that can be directly measured, acceptance-tested, and falsified:

1. **Calibration**: when the model says "I am 80% confident," is it actually right 80% of the time?
2. **Selective abstention**: declining to answer on high-risk inputs, with the quality of refusal judged by a risk–coverage curve.
3. **Identifiability awareness**: knowing which latent variables cannot, in principle, be estimated under the current observation setup, and refusing to guess them.

We report three sets of direct evidence from a small world model of 967,796 parameters. First, split conformal prediction passes coverage at all three nominal levels, 80%/90%/95% (observed 0.9577 at the 95% level), including a "blind" path that cannot see the latent parameters. Second, estimating acceleration from a short 6-frame window yields an estimator whose error is 50.8% worse than simply predicting the mean (skill score −0.508) — the correct action is to abstain or lengthen the observation window, not to force an estimate. Third, all 21 release gates pass; on contact tasks, direct control without a motion prior succeeds 0% of the time, rising to 100% once a gate routes those tasks to a hybrid policy.

These are single-seed preliminary results on a CPU prototype, not laws validated on real large models. The real contribution is a research program: replacing the unmeasurable word "wisdom" with three measurable engineering prerequisites, together with five experimentally falsifiable predictions. Until these three things are measurable, acceptance-tested, and falsifiable, any claim that a system "has wisdom" is not rigorous.

**Keywords**: metacognition; hallucination; calibration; selective prediction; conformal prediction; structural identifiability; world models; safety gating

---

## 1 A Car with a Huge Engine and No Brakes

### 1.1 Confucius as a diagnostic report

In the *Analects* (II.17), Confucius says: **"To know what you know and what you do not know, that is true knowledge."** The sentence is not about the amount of knowledge; it is about awareness of one's own epistemic state: knowing what you know is clarity; knowing what you do not know is wisdom.

Run today's AI through that diagnostic and the result is striking. In sheer volume of knowledge, a large model has read more text than any human could encounter in a lifetime. In self-awareness it remains an infant: it does not know where its answers come from, how reliable they are, or where its knowledge ends.

One rule must be set first: **Confucius's sentence is a moral ideal (an ought), not scientific evidence (an is).** We cite it only to name the ancient intuition we are trying to engineer. Every scientific claim in this paper rests on the three measurable quantities of Section 2 and the experiments of Section 4. If our predictions are experimentally refuted, Confucius cannot save the paper.

### 1.2 A precise definition of hallucination

Hallucination is often defined loosely as "the model says something wrong." That definition is too broad — every finite model errs. We adopt a sharper and more dangerous definition:

> **Hallucination is when a model outputs, for something it fundamentally does not know, an answer every bit as confident as its answer to something it does know.**

The first (being wrong) is a competence deficit; the second (being confidently wrong) is a missing self-boundary. A self-aware system, when out of its depth, should emit a wide interval, abstain, or go retrieve information; a system without self-awareness still hands back a firm point estimate.

Why does a probability matcher lack a boundary by construction? Because its training objective — predict the next token — contains no dimension for "do I actually know this?" It is trained always to produce the most likely continuation, never to judge whether it should produce one at all.

### 1.3 What this paper does and does not do

This paper does **not** claim that AI has consciousness, a self, or subjective experience. Calibration, abstention, and identifiability awareness are all behavioral, engineering quantities; a machine can perform all of them with no inner experience whatsoever (Section 6 faces this objection head-on).

What the paper **does** is find three engineering prerequisites for "self-knowledge" and make them measurable, acceptance-tested, and falsifiable. We are not building the engine — ability is the subject of P0/P0b — we are installing the **brakes**. Brakes produce no power, but a brakeless car becomes more dangerous the more powerful its engine is.

---

## 2 Taking "Self-Knowledge" Apart into Three Measurable Things

"Knowing what you know" sounds mystical; taken apart, each piece has mature mathematical tooling.

### 2.1 Calibration: 80% confident should mean 80% correct

**Calibration** asks whether stated confidence matches actual correctness. A model that says "90% confident" on 100 questions and gets exactly 90 right is perfectly calibrated; if it gets 55 right, it is overconfident.

Standard tools are the Expected Calibration Error (ECE — bin samples by confidence and compare each bin's mean confidence with its accuracy) and the Brier score (mean squared error between confidence and correctness). **The key point: calibration and accuracy are different things.** A 70%-accurate model can be well calibrated (it knows it is right only seven times in ten), while a 95%-accurate model can be badly calibrated (it always claims certainty). Guo et al. (2017) found that modern deep networks are systematically overconfident, and that a one-parameter post-processing step called temperature scaling substantially improves calibration with almost no change in accuracy — evidence that "knowing whether you are right" and "being right" are separable axes.

### 2.2 Selective abstention: permitting the words "I won't answer"

**Selective classification** adds an abstention switch: on inputs whose confidence falls below a threshold, the model does not answer and hands off to a human or another system.

Its central tool is the **risk–coverage curve**: coverage is the fraction of samples the model answers; risk is its error rate on the answered samples. Sweeping the confidence threshold from loose to tight lowers both coverage and risk, and the curve should sit as far toward the lower-left as possible. A scalar summary is the area under this curve, AURC, where **smaller is better**. The framework goes back to Chow's (1957) optimum error–reject trade-off and was formalized by El-Yaniv & Wiener (2010) and Geifman & El-Yaniv (2017).

More abstention is not always better — in medicine or autonomous driving, "I don't know" can be more dangerous than a wrong answer when it hands control back to an already-overloaded human. A good system lets the operator make an explicit trade-off under a risk budget rather than simply refusing whenever uncertain.

### 2.3 Identifiability awareness: some things are unknowable under the current observations

The first two quantities ask "how sure am I of this answer?" The third is more fundamental: **can this quantity be estimated at all under the current observation structure?**

Systems identification (Bellman & Åström, 1970) distinguishes:

- **Observability**: can the internal state be recovered from an observation trajectory? A yes/no structural property.
- **Identifiability**: given the data, can the unknown parameters be estimated accurately enough? A question of "how well."

A quantity can be highly observable (its shadow is present in the data) yet remain non-identifiable (the shadow is too weak or the sample too short, so the estimator is worse than not modeling at all). **A self-aware system must detect this and refuse to guess** — the piece this paper most wants to add to the classical calibration literature. We quantify it with a skill score:

$$\text{skill}=1-\frac{\text{estimator error}}{\text{error of always predicting the mean}} \tag{1}$$

A positive score means the model beats the naive mean; zero means it learned nothing; **a negative score means the model is worse than doing nothing — and the rational action is not to use it.**

### 2.4 The triad: a minimal engineering framework

Putting the three together, we describe an "engineering self-model" as a triad:

$$\big(M_{\text{self}},\ U,\ A(x,\tau)\big) \tag{2}$$

- $M_{\text{self}}$ (self-model): the system's estimate of its own competence — how it has historically performed on this kind of task;
- $U$ (uncertainty): the uncertainty attached to the current prediction, distinguishing **aleatoric uncertainty** (noise intrinsic to the data, irreducible with more data) from **epistemic uncertainty** (uncertainty from not having seen enough, removable by collecting more data);
- $A(x,\tau)$ (abstention policy): abstain, hand off, or call a tool when uncertainty exceeds threshold $\tau$.

Uncertainty must also come with **coverage-guaranteed intervals**, not just points; conformal prediction provides them (Section 4.2).

A note on levels. Cognition can be coarsely layered: L1 perception (pattern recognition), L2 cognition (reasoning and planning), L3 metacognition (knowing what you do and do not know), L4 hyper-cognition (recalibrating your own calibration, even posing new questions). This paper does L3 only; L4 is open. The four-layer scheme is our organizing frame, not a consensus of the literature.

![Figure 1 The metacognition loop](figures/P0c_fig1_metacog_loop.png)

> Figure 1: The metacognition loop. After acting, the system observes the outcome; the world model produces a prediction and an uncertainty; calibration/identifiability modules judge "do I know this?"; the abstention policy decides whether to answer, refuse, or go external; the result is written back into the self-model.

---

## 3 The Road Already Traveled

This paper is not built in a vacuum. Four research lines have, over decades, shaped the three pieces.

**The concept of metacognition** comes from developmental psychology. Flavell (1979) coined the term and divided metacognition into metacognitive knowledge (understanding one's own cognitive processes) and metacognitive monitoring (watching one's current state of comprehension). We borrow the engineering counterpart of the latter: online monitoring of the reliability of one's own predictions.

**Calibration** was rediscovered in the deep-learning era. Guo et al. (2017) showed modern networks are overconfident and that temperature scaling (dividing logits by a scalar $T$ before softmax) improves calibration cheaply without changing predicted classes — direct support for prediction F1.

**Abstention** has a complete theoretical line: Chow (1957) introduced optimal recognition with a reject option; El-Yaniv & Wiener (2010) founded the risk–coverage framework for noise-free selective classification; Geifman & El-Yaniv (2017) built a selective classifier for any trained network; SelectiveNet (2019) baked the reject option into the network itself.

**Distribution-free uncertainty quantification** is supplied by conformal prediction. The Angelopoulos & Bates (2021) primer systematizes split conformal: compute a quantile on calibration data untouched by training, and prediction intervals inherit a coverage guarantee that assumes nothing about the distribution — only exchangeability between calibration and test samples.

**Do large models know what they know?** Kadavath et al. (2022) ran the key experiment: models' non-verbal ability to predict their own correctness in multiple-choice settings correlates with actual accuracy but is far from perfect, and RLHF fine-tuning harms calibration. Reflexion (Shinn et al., 2023) has agents write reflections into language memory. World Models (Ha & Schmidhuber, 2018) suggests another route: use prediction error itself as the signal for "I have not seen this." Hendrycks & Gimpel (2017) found the maximum softmax value to be a cheap baseline for out-of-distribution detection.

Together these leave a gap: **the calibration literature rarely asks whether a quantity is structurally estimable; the identifiability literature rarely connects to LLM abstention engineering; conformal prediction is rarely placed inside a unified "metacognition" frame.** This paper joins the three pieces and runs them end-to-end on a controllable small world model.

---

## 4 What We Actually Measured

The platform is a small world model inside the UDOS engine: a 2-layer gated recurrent network, hidden size 256, 32-dimensional scene latent, 6-frame input windows, **967,796 parameters** in total. It learns the state–action–next-state transition in a synthetic 2-D dynamical world deliberately containing four regimes: uniform motion, acceleration, springs, and collisions.

### 4.1 The world model itself: first prove it can predict

From v7.0.1 to v7.0.3, overall prediction error (MSE) on the blind test set fell from 0.0459 to **0.01037**, roughly a 4.4× improvement. By regime, collisions are easiest (0.00304), then uniform motion (0.00573), acceleration (0.01410), and springs hardest (0.01862). Median single-step prediction latency (batch size 1) is 2.36 ms; a 4-step rollout takes 4.83 ms — light enough for a real-time control loop. (Level verified.)

This step is the precondition for everything else: **a model that cannot predict cannot assess how good its predictions are.**

### 4.2 Conformal prediction: promise 95%, observe 95.8%

We wrap the world model's predictions in split-conformal intervals: 640 windows and 128 trajectories serve as a held-out calibration set; on fresh test trajectories we check how often the true value falls inside. Two paths are run: an oracle path allowed to see the true latent parameters, and a blind path that cannot (closer to real deployment).

**Table 1 Conformal coverage: nominal vs observed (level verified, ±0.05 tolerance)**

| Nominal | Oracle coverage | Oracle width | Blind coverage | Blind width |
|---|---|---|---|---|
| 80% | 0.7839 | 0.0430 | 0.7758 | 0.0504 |
| 90% | 0.8984 | 0.0858 | 0.9001 | 0.1081 |
| 95% | **0.9577** | 0.1612 | **0.9559** | 0.2052 |

![Figure 2 Conformal coverage](figures/P0c_fig2_conformal_coverage.png)

> Figure 2: At all three nominal levels, observed coverage lies within ±0.05 (all 21 release gates pass). The blind path, which cannot see latent parameters, still meets coverage, at the price of systematically wider intervals (0.2052 vs 0.1612 at 95%).

**How to read this table.** The model need not "feel" how certain it is; it simply measures a quantile once on calibration data and obtains intervals with a mathematical guarantee — "when I say 95%, roughly 95% of future true values fall in." The wider blind intervals matter most: **with less information the system automatically becomes more humble — the germ of boundary awareness.**

The conformal guarantee has an explicit premise: calibration and test data must be exchangeable. Under distribution shift or out-of-distribution inputs the premise fails and coverage degrades — the subject of prediction F4, not a limitation we paper over.

### 4.3 The acceleration negative result: when guessing is worse than not guessing

This is the most counterintuitive and valuable result. We ask the estimator to recover four latent physical quantities from a 6-frame window, 160 samples each, and compute the skill score of Eq. (1):

**Table 2 Identifiability of four latent quantities (n=480; main gate verified, breakdown cpu-proto)**

| Latent quantity | Estimator error | Mean-prediction error | Skill | Observability |
|---|---|---|---|---|
| Initial velocity v0 | 0.6606 | 0.9683 | **+0.318** | 0.771 |
| Angular frequency ω | 0.1471 | 0.2563 | **+0.426** | 0.914 |
| Collision restitution v2 | 0.0432 | 0.2396 | **+0.820** | 0.908 |
| Acceleration a (6-frame window) | 0.9256 | 0.6138 | **−0.508** | 0.798 |

![Figure 4 Identifiability awareness](figures/P0c_fig4_identifiability_skill.png)

> Figure 4: The first three quantities have positive skill — the model genuinely learns them. Acceleration under a 6-frame window scores −0.508: the estimator is 50.8% worse than predicting the mean. Note that its observability is not low (0.798) — acceleration's shadow is in the data, but six frames are too few to identify it.

**The negative result says three things.** First, high observability does not imply identifiability: the shadow of acceleration is present, but the window is too short for any model to estimate it. Second, a system without self-awareness would here confidently emit that terrible point estimate — a textbook hallucination; a self-aware system that sees negative skill should abstain or extend the window from 6 to 12/24 frames. Third — and this is the core of prediction F2 — **this is a hard boundary of the observation structure, not something a bigger model routes around.**

### 4.4 Gating: wiring "I don't know" into real actions

Calibration and intervals produce no safety value if they stay in reports. We wire uncertainty into a set of pre-release gates, 21 checks in total, all passing (coverage tolerance ±0.05, parameter fan ±0.10).

Two gates make the point most clearly:

- **Contact gate**: in embodied hybrid-control experiments (40 episodes per arm, level cpu-proxy), pure semantic control of the arm succeeds 75% with collision rate 0.25 and 1.0 takeover on average; motion-prior-only control succeeds 52.5%; on contact tasks, direct control without a motion prior succeeds **0%** of the time. Once the gate routes contact tasks to the hybrid policy, it reaches **100%** success, zero collisions, and only 0.047 takeovers. **The gate does not make the model smarter; it stops it from forcing its way through tasks it is bad at.**
- **Kill switch**: when the retry budget is set to zero, the system honestly refuses the task, the governance check returns not-ok and it degrades — it does not fake completion to "finish the job."

Gate trigger rates carry information too: uniform motion 63.4%, acceleration 66.2%, collisions 15.4%, springs 0%. The model calls for help more often on the dynamics it finds harder to identify, consistent with the ordering in Section 4.3.

### 4.5 The collective level: brakes for one agent and for a million

In multi-agent systems the self-boundary has a collective meaning: the whole swarm must also know whether it can still converge. Three results from companion papers:

- A Byzantine-fault-tolerant committee with n=7, f=2 decides all 24 work orders correctly (quorum 2f+1);
- Under fault injection, a system with the Owner/Trace/Stop-Condition governance matrix converges 24/24 in both clean and stressed runs, and isolates 3 faulty agents through 5 reworks (**single-seed pilot; ≥30 seeds required**);
- The internal task market conserves budget, payment, and balance exactly (240=240=240); conservation is itself a global self-check that the system state remains consistent.

### 4.6 Evidence summary

**Table 3 Mapping the three core evidence sets to the metacognition triad**

| Triad component | Experiment | Key number | Level |
|---|---|---|---|
| Uncertainty U (calibration) | Split conformal | 0.9577 oracle / 0.9559 blind at 95% | verified |
| Self-model M_self (identifiability awareness) | Four-parameter identification | Acceleration skill −0.508 → abstain | main gate verified |
| Abstention policy A(x,τ) | 21 gates | All pass; contact tasks 0%→100% | verified/cpu-proxy |

---

## 5 How This Paper Could Be Proved Wrong

We keep the three most decisive of five predictions, each with a minimal experiment and a decision rule.

**F1 | Calibration and performance are separable axes.**
*Minimal experiment:* take one trained checkpoint (no retraining), freeze the weights, do a one-dimensional grid search over temperature $T$ on a calibration set, and measure ECE and accuracy with 5-fold cross-validation. *Decision rule:* F1 holds if some $T$ cuts ECE by ≥30% relative while accuracy moves within noise (<0.1%); F1 dies if every measurable calibration gain comes with a proportional accuracy gain (calibration is a mere by-product of performance).

**F2 | When the observation structure is insufficient, more parameters cannot rescue it.**
*Minimal experiment:* hold the 6-frame window fixed, only double and quadruple estimator capacity (hidden 256→512→1024, layers 2→4), and measure acceleration skill; run a positive-control arm that extends the window to 12/24 frames. *Decision rule:* F1 holds if skill stays ≤0 as capacity grows but turns positive immediately with a longer window — cleanly separating observation structure from model capacity; F2 dies if a big model lifts skill stably above +0.2, meaning scale does break the structural boundary and the paper retreats to "a small-model constraint." This is the boldest and most easily tested prediction.

**F3 | Gating answers more under the same error budget.**
*Minimal experiment:* on one benchmark run two arms — bare model vs the same model plus conformal/confidence gating; fix a target error rate, sweep the threshold, compare coverage and AURC. *Decision rule:* F3 holds if the gated arm covers significantly more (significantly smaller AURC); F3 dies if the two risk–coverage curves coincide within statistical noise (abstention merely passes hard problems to someone else).

Two secondary predictions: **F4**, explicit uncertainty channels (conformal intervals, parameter fans) should degrade more slowly than verbalized confidence under out-of-distribution shift — Kadavath already found verbal confidence imperfect and harmed by RLHF; **F5**, governed multi-agent systems should converge more often under fault injection, but the current 24/24 is a single-seed point estimate and **must be rerun with ≥30 seeds with a 95% confidence interval**; if that interval contains zero, F5 dies.

---

## 6 Where We Might Be Wrong

**First, calibration is not consciousness.** Temperature scaling divides by a scalar; conformal prediction looks up a quantile; a gate compares two numbers — none of this requires the system to "reflect on its own reflection." We fully accept the behaviorist objection and put it in the title: our "metacognition" is an engineering term denoting the three behavioral quantities, not Flavell's metacognitive experience, let alone consciousness. A system can be perfectly calibrated with no subjective experience at all. Reducing self-knowledge from a question of inner experience to one of behavioral measurement is precisely what lets us build it.

**Second, over-abstention is another kind of failure.** A system that refuses at the slightest doubt is useless; in a doctor-scarce setting, handing the patient back, or handing control back to a distracted driver, can be worse than answering. Confidence can also be fooled by adversarial inputs, and conformal exchangeability fails out of distribution. Our AURC currently measures only "how much answered vs how much wrong," not "how much refused and at what cost" — modeling the cost of refusal is explicit future work.

**Third, introspection itself is unreliable.** Asking a model to state its own confidence produces a self-report that itself needs calibrating; Reflexion-style verbal reflection can mis-attribute a failure and write the wrong cause into memory, contaminating the next trial. Hence our one-line stance: **do not trust what it says; trust what it measures.** Internal confidence signals may be candidates, but they must be calibrated against an independent calibration set.

**Fourth, Confucius is not evidence, and a synthetic world is not the real world.** The *Analects* only motivates; every UDOS number comes from a deterministic CPU prototype (point-mass dynamics, rule-based judges, a single seed) — not a real large model, not real hardware, with no 95% confidence intervals. The acceleration negative result was measured on an injected short-window defect; real-world "insufficient observation" is more complex. We therefore position this paper as **a research program plus preliminary silicon evidence**: it shows the mechanism's shape works in a simplified world, not that its constants hold in real systems. Moving conformal prediction onto real LLM output spaces and rerunning every pilot with ≥30 seeds are the two things that must be done before submission.

---

## 7 Consciousness: Explicitly Out of Scope

This chapter marks the boundary. The paper nowhere **claims** consciousness, qualia, or a sense of self. Theories such as Attention Schema Theory (AST) or Global Workspace Theory (GWT) superficially resemble our self-model, but we **neither endorse, cite, nor test** them: they are hypotheses about human consciousness whose applicability to silicon is itself contested, while our $M_{\text{self}}$ is merely a numeric estimate of the system's own competence — not an "attention schema."

A calibrated system need not be conscious; a conscious system need not be calibrated. The existence of this chapter is itself the discipline of "knowing what you do not know": **this paper knows it does not know what consciousness is, and does not pretend to.**

---

## 8 Engineering Implications: Install the Brakes Before Flooring the Throttle

**First, calibration should be a first-class acceptance criterion, not a post-launch patch.** Temperature scaling is cheap, but only if you actually measure it. Writing ECE, coverage, and AURC into CI gates (as our 21 gates do) is far cheaper than firefighting afterward.

**Second, hallucination is to be managed, not fantasized away.** No finite system is right on every sample; what you can do is ensure that, when it does not know, it does one of three things: emit a wide interval (conformal), abstain (gating), or retrieve external knowledge (RAG/Toolformer). "Eliminate hallucination with a bigger model" works only in-distribution — the root of hallucination is a missing self-boundary, not insufficient scale, in direct correspondence with prediction F2 of P0b.

**Third, identifiability must be considered at observation-design time.** The acceleration result tells architects: if the window is too short or the sensors cannot see a key quantity, no larger model helps; the correct move is a longer window or better sensors, not more parameters.

**Fourth, the trilogy is multiplicative.** P0 says structure determines the shape of the competence function; P0b says scale determines where you stand on the curve; this paper says the self-boundary determines where you must stop. Structure and scale are the engine; the self-boundary is the brake: a brakeless car is more dangerous the more power it has, while a car with only brakes and no engine is just a roadblock. All three are required.

---

## 9 Conclusion

P0 says: space structure is itself intelligence. P0b says: structure plus scale is itself intelligence. This paper says: **knowing what one does not know is the third axis of intelligence.**

We have not proved that any system possesses wisdom. What we did is far more modest: we replaced the unmeasurable word "wisdom" with three measurable engineering prerequisites — calibration, abstention, identifiability awareness — ran them end-to-end on a 968k-parameter world model, and set up five targets that experiments can knock down.

AI's coming-of-age will not be the day it answers every question. It will be the day it first honestly says, "I don't know — let me look that up," or "this lies beyond what my observations can determine." The *Analects* held that ideal for 2,500 years; what this paper does is convert a moral precept into a measurable, acceptance-testable, falsifiable engineering proposition. Until the three things are built, calling a system "wise" is not rigorous — however fluent its answers.

---

## References

> AI/systems references reuse the volume's master bibliography (checked 2026-09-19); the entries below were checked individually on 2026-09-19/20.

1. Flavell, J. H. (1979). Metacognition and Cognitive Monitoring. *American Psychologist*, 34(10), 906–911. DOI:10.1037/0003-066X.34.10.906.
2. Guo, C., Pleiss, G., Sun, Y. & Weinberger, K. Q. (2017). On Calibration of Modern Neural Networks. *ICML*, PMLR 70:1321–1330. arXiv:1706.04599.
3. Chow, C. K. (1957). An Optimum Character Recognition System Using Decision Functions. *IRE Trans. Electronic Computers*, EC-6, 247–254.
4. Chow, C. K. (1970). On Optimum Recognition Error and Reject Tradeoff. *IEEE Trans. Information Theory*, IT-16 (exact pages `[CITATION NEEDED]`).
5. El-Yaniv, R. & Wiener, Y. (2010). On the Foundations of Noise-free Selective Classification. *JMLR*, 11, 1605–1641.
6. Geifman, Y. & El-Yaniv, R. (2017). Selective Classification for Deep Neural Networks. *NeurIPS*.
7. Geifman, Y., Uziel, G. & El-Yaniv, R. (2019). SelectiveNet. arXiv:1901.09192.
8. Angelopoulos, A. N. & Bates, S. (2021). A Gentle Introduction to Conformal Prediction. arXiv:2107.07511.
9. Kadavath, S. et al. (2022). Language Models (Mostly) Know What They Know. arXiv:2207.05221.
10. Shinn, N. et al. (2023). Reflexion: Language Agents with Verbal Reinforcement Learning. *NeurIPS*. arXiv:2303.11366.
11. Ha, D. & Schmidhuber, J. (2018). World Models. arXiv:1803.10122.
12. Hendrycks, D. & Gimpel, K. (2017). A Baseline for Detecting Misclassified and Out-of-Distribution Examples. *ICLR*. arXiv:1610.02136 (abstract fields `[CITATION NEEDED]`).
13. Lewis, P. et al. (2020). Retrieval-Augmented Generation. *NeurIPS*. arXiv:2005.11401.
14. Schick, T. et al. (2023). Toolformer. *NeurIPS*. arXiv:2302.04761.
15. Bellman, R. & Åström, K. J. (1970). On Structural Identifiability. *Mathematical Biosciences* (exact pages `[CITATION NEEDED]`).
16. Ji, Z. et al. (2023). Survey of Hallucination in Natural Language Generation. *ACM Computing Surveys* (exact pages `[CITATION NEEDED]`).
17. Castro, M. & Liskov, B. (1999). Practical Byzantine Fault Tolerance. *OSDI*.

---

## Appendix: Provenance of the UDOS Numbers

| Number | Data file | Level |
|---|---|---|
| World model 967,796 params; blind MSE 0.0459→0.01037; latency 2.36 ms | `v7_verification.json` (`params`/`improvement_vs_v701`/`latency_ms`) | verified |
| Conformal 80/90/95: oracle 0.7839/0.8984/0.9577, blind 0.7758/0.9001/0.9559; widths in Table 1 | `v7_verification.json` (`coverage.*`) | verified |
| Skill: v0 +0.318, ω +0.426, v2 +0.820, acceleration −0.508 (n=480) | `v7_verification.json` (`estimator_param_error.*`) | main gate verified, breakdown cpu-proto |
| 21 gates all pass; trigger rates 0.6341/0.6624/0.1544/0 | `v7_verification.json` (`gating`) | verified |
| Embodied hybrid direct 75%/motion 52.5%/hybrid 100%; collisions 0.25→0; takeovers 1.0→0.047 | `embodied_hybrid_demo.json` (P7) | cpu-proxy |
| BFT n=7,f=2 all correct; market conservation 240; P3 24/24 (single seed) | P2/P3/P11 sources | cpu-proto |

*(End. P0c is the prologue "self-boundary" axis of the flagship trilogy; the structure axis is P0, the scale axis P0b.)*


---

# Flagship Paper P0d: Why Swarm Intelligence Is the Key to AGI

> **Archival and editorial conventions**
>
> This document is the complete archive of Agent Universe flagship paper P0d, **preserving the three rounds of exposition verbatim, with no deletion and no rewriting**.
>
> The "quantified benefits of swarm memory" table that appears in the three rounds (task hit rate 60%→85%, first-attempt success rate 45%→72%, average attempts 3.2→1.8, long-tail coverage 30%→65%, cold start from days to hours −80%, etc.) had, in the original exposition, **no experiment, no source, and no defined scope — it was an assertion**, and is the weakest point of the entire text in terms of evidence. That table has been replaced by a **reproducible minimal controlled experiment**: the authoritative measured data, scope, reproduction commands, and evidence boundary are given in **Appendix A** at the end; the corresponding passages in the original text are **preserved as-is and annotated with "for correction see Appendix A"**, without deleting or altering the original figures.
>
> - Archive date: 2026-09-24
> - Corresponding experiment code commit: `4287d48` (pushed to agent-universe)
> - Evidence grade: the mathematical derivation of network topology is cpu-proto; the memory controlled experiment is cpu-proto (a synthetic controlled experiment)

---

## Opening outline

1. The network-structure Scaling Law of Agents
2. Intelligence distillation — from a single model & a single agent to group deliberation, collaboration, and coordination, making swarm intelligence higher, larger, and stronger than individual intelligence & more accurate, more reliable, more stable, and able to handle more complex tasks & problems & scenarios faster
3. The shared swarm-memory mechanism — although pretraining is not possible, post-training can be performed to increase hit rate & success rate

---
---

# Round 1: Swarm Intelligence — A Technical Deconstruction of the Three Major Advantages

## I. The Network-Structure Scaling Law of Agents — From "Individual Intelligence" to "Network Intelligence"

### 1.1 Underlying logic

The large-model Scaling Law (parameters × data × compute) solved the problem of "whether intelligence exists," but it carries an implicit assumption: intelligence is a property of a single model. Swarm intelligence breaks this assumption — intelligence is a property of network structure.

The core conclusion of UDOS P0b is pushed to its extreme here:

Capability ≈ F(structure S; N, D, C)

When the basic unit changes from a Token to an Agent, the three axes of the Scaling Law undergo a qualitative change:

| Scaling axis | Single-model Scaling | Agent-network Scaling |
|---|---|---|
| N (nodes) | Number of parameters | Number of Agents |
| D (data) | Training Tokens | Number of inter-Agent interactions |
| C (compute) | FLOPs | Network communication bandwidth × collaboration rounds |
| Structure S | Attention connectivity | Network topology (star / hierarchical / mesh) |

**Key finding (from P0b, evidence one)**: with the same one million nodes, the center of a star topology must process 20.97 million messages, while in a hierarchical topology each node needs only 9. The growth exponent changes from 1 to 0.

This is not "a bit more efficiency"; rather, the Scaling Law of network intelligence has a fundamentally different mathematical form from the single-model Scaling Law.

### 1.2 The three Scaling curves of network structure

| Topology | N=10 | N=1000 | N=10⁶ | Growth exponent | Applicable scenario |
|---|---|---|---|---|---|
| Star | Center load 10 | Center load 1000 | Center load 10⁶ | β=1 (linear explosion) | Small scale, strong center |
| Hierarchical | Fan-in ≤9 | Fan-in ≤9 | Fan-in ≤9 | β=0 (constant) | Large scale, decentralized |
| Mesh | 45 connections | 500,000 connections | 10¹¹ connections | β=2 (quadratic explosion) | Small scale, high coordination |

**Conclusion**: only hierarchical/hybrid topologies can support the Scaling of million-level and hundred-million-level Agent networks. A star topology is already physically infeasible at N=10⁴, and a mesh topology has out-of-control communication cost at N=10³.

### 1.3 The "phase-transition point" of network intelligence

The precise correspondence of P0b's three-stage curve in an Agent network:

| Stage | Characteristic | Agent-network manifestation | Breakthrough condition |
|---|---|---|---|
| Stage 1 | Power-law growth | Increasing the number of Agents → linear improvement in capability | Correct structure, small scale |
| Stage 2 | Structural phase transition | The star cannot hold → must switch to hierarchical | Communication wall, coordination wall |
| Stage 3 | Saturation/negative return | Homogeneous Agent expansion → dividends go to zero | Must introduce independent information sources |

P9's quality-saturation law gives the precise mathematical form here:

mse(k) = a + b/k

- a = the irreducible floor (determined by the strongest member)
- b = the slope of the diversity dividend (determined by member independence)
- When k exceeds the number of truly independent sources D, b→0, and adding people is ineffective

This means that the Scaling Law of network intelligence is not "the more people the better," but rather **"structure determines the shape of the curve, scale determines which stage you stand on."**

## II. Intelligence Distillation — The Qualitative Change from "Monolithic Intelligence" to "Swarm Intelligence"

### 2.1 Why swarm intelligence is stronger than individual intelligence

Bottlenecks of single-model intelligence:

| Bottleneck | Manifestation | Root cause |
|---|---|---|
| Capability ceiling | GPT-5 is still inferior to human experts on some tasks | The training data of a single model is biased |
| Reliability | Hallucinations and erroneous reasoning cannot be self-corrected | There is no external verification mechanism |
| Stability | Multiple answers to the same question are inconsistent | The randomness of probabilistic sampling |
| Complexity | Long-horizon tasks easily "get lost" | The context window is limited |
| Interpretability | Black-box decisions cannot be traced | The weight distribution is unreadable |

The qualitative change of swarm intelligence:

| Dimension | Single model | Swarm intelligence | Improvement mechanism |
|---|---|---|---|
| Accuracy | Single-point estimate | Calibrated weighting of multiple experts | P9: calibrated-weight MSE 0.0092 vs equal-weight 0.2131 |
| Reliability | Single point of failure | BFT-lite fault tolerance | P2: n≥3f+1, 24/24 correct under 2 Byzantine nodes |
| Stability | Random fluctuation | Consensus convergence | P2: equivocation invalidates the entire round |
| Complexity | Limited context | Task decomposition + handoff | P10: the six fields of TransferBundle |
| Interpretability | Black box | TraceLedger hash chain | P12: evidence grading + traceability |

### 2.2 The three modes of intelligence distillation

**Mode 1: Deliberation distillation (Debate & Consensus)**

```
Multiple Agents reason independently → exchange intermediate conclusions → debate → vote → converge to the optimal solution
```

| Mechanism | Implementation | Effect |
|---|---|---|
| Independent reasoning | Different models / different prompts / different data | Guarantees diversity |
| Intermediate exchange | Share the Trace (not the full context) | Avoids homogenization |
| Debate | Adversarial verification | Exposes erroneous reasoning |
| Voting | A BFT-lite committee | Prevents single-point errors |
| Convergence | Calibrated weighting | The optimal solution wins |

**Mode 2: Collaboration distillation (Divide & Conquer)**

```
Complex task → decompose into subtasks → assign to expert Agents → execute in parallel → aggregate results
```

| Mechanism | Implementation | Effect |
|---|---|---|
| Task decomposition | Matching based on skill tags | Each subtask finds the optimal Agent |
| Parallel execution | Near-linear scaling for I/O-bound work | 7.986× speedup (P9) |
| Handoff | The six fields of TransferBundle | No information is lost |
| Aggregation | Weighted ensembling | Quality saturates at the best member |
| Rework | Circuit breaker + reassignment | Failures do not affect the whole |

**Mode 3: Coordination distillation (Stigmergy & Emergence)**

```
Agents collaborate indirectly through an environmental medium (a shared blackboard) → no peer-to-peer communication needed → group behavior emerges
```

| Mechanism | Implementation | Effect |
|---|---|---|
| Environmental medium | Shared blackboard / shared memory | Communication volume 240→60 (P0b, evidence six) |
| Indirect collaboration | Leaving traces + reading them | No need to know who did what |
| Self-organization | Local rules → global order | Capabilities beyond individuals emerge |
| Scalability | Communication O(1) rather than O(N²) | Supports hundred-million-level Agents |

### 2.3 The "qualitative-change critical point" of intelligence distillation

Three thresholds from quantitative to qualitative change:

| Threshold | Condition | Qualitative-change manifestation |
|---|---|---|
| Diversity threshold | Number of independent sources D≥3 | Ensemble error begins to drop markedly |
| Scale threshold | Number of Agents k≥10 | Swarm intelligence emerges (surpassing the strongest individual) |
| Coordination threshold | Collaboration depth ≥3 layers | The completion rate of complex tasks jumps |

Precise data from P9:

- k=1: MSE 0.009196 (the best single expert)
- k=2: MSE 0.0092 (almost no improvement)
- k=3-17: MSE fluctuates around 0.0092 (homogeneous replicas are ineffective)
- But if a truly independent third source is introduced: MSE drops markedly

**Conclusion**: the qualitative change of swarm intelligence comes not from "more Agents," but from **"more independent information sources" + "better collaboration structures."**

## III. The Shared Swarm-Memory Mechanism — Post-training Improvement of "Hit Rate & Success Rate"

### 3.1 Why pretraining is impossible but post-training is possible

| Dimension | Pretraining | Post-training (shared swarm memory) |
|---|---|---|
| Data source | Centralized massive data | Distributed Agent experience |
| Update method | Weight update | Memory-base update + fine-tuning |
| Cost | Extremely high (ten-thousand-GPU clusters) | Low (incremental updates) |
| Timeliness | Frozen after training | Updated in real time |
| Generalization | Strong | Weak (but precise) |
| Applicable scenario | Foundational capabilities | Specific-task optimization |

**Core insight**: pretraining solves "general capability," while swarm memory solves "hit rate in specific scenarios." The former is a "general practitioner," the latter a "specialist + case library."

### 3.2 The architecture of swarm memory

```
┌─────────────────────────────────────────────────────────────────┐
│                 Shared Swarm-Memory Architecture                │
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│  │  Agent A    │    │  Agent B    │    │  Agent C    │        │
│  │ Experience  │    │ Experience  │    │ Experience  │        │
│  │ generation  │    │ generation  │    │ generation  │        │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘        │
│         │                  │                  │                │
│         └──────────────────┼──────────────────┘                │
│                            ↓                                   │
│              ┌─────────────────────────┐                       │
│              │   Swarm memory base     │                       │
│              │     (shared)            │                       │
│              │  ┌─────────────────┐    │                       │
│              │  │ Success-case    │    │                       │
│              │  │ library         │    │                       │
│              │  │ Failure-case    │    │                       │
│              │  │ library         │    │                       │
│              │  │ Boundary-       │    │                       │
│              │  │ condition lib.  │    │                       │
│              │  │ Best-practice   │    │                       │
│              │  │ library         │    │                       │
│              │  └─────────────────┘    │                       │
│              └────────────┬────────────┘                       │
│                           ↓                                    │
│              ┌─────────────────────────┐                       │
│              │   Post-training engine  │                       │
│              │ (incremental fine-tune  │                       │
│              │  + prompt optimization) │                       │
│              └────────────┬────────────┘                       │
│                           ↓                                    │
│         ┌──────────────────┼──────────────────┐                │
│         │                  │                  │                │
│  ┌──────┴──────┐    ┌──────┴──────┐    ┌──────┴──────┐        │
│  │  Agent A    │    │  Agent B    │    │  Agent C    │        │
│  │ Capability  │    │ Capability  │    │ Capability  │        │
│  │ improvement │    │ improvement │    │ improvement │        │
│  └─────────────┘    └─────────────┘    └─────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 The four types of swarm memory

| Memory type | Content | Source | Use |
|---|---|---|---|
| Success-case library | Task → solution → result | Successful executions of all Agents | Direct reuse for similar tasks |
| Failure-case library | Task → attempt → failure reason | Failure records of all Agents | Avoid repeating the same pitfalls |
| Boundary-condition library | Task → parameter range → applicability | Systematic testing | Determine whether a task is solvable |
| Best-practice library | Task → optimal strategy → validation | Repeated comparative executions | Continuously optimize strategy |

### 3.4 The three paths of post-training

**Path 1: Prompt optimization**

```
Swarm memory → extract optimal prompt patterns → update the Agent's system prompt
```

| Step | Content | Effect |
|---|---|---|
| 1 | Collect prompts from success cases | Find "what prompts work" |
| 2 | Extract common patterns | Form prompt templates |
| 3 | Inject into the Agent's system prompt | Improve hit rate |

**Path 2: Memory injection**

```
Swarm memory → vectorize → store in the Agent's memory base → retrieve at inference time
```

| Step | Content | Effect |
|---|---|---|
| 1 | Encode success cases as vectors | Retrievable |
| 2 | Store in the Agent's long-term memory | Scalable |
| 3 | Similarity retrieval at inference time | Improve success rate |

**Path 3: Incremental fine-tuning**

```
Swarm memory → construct training data → LoRA fine-tuning → weight update
```

| Step | Content | Effect |
|---|---|---|
| 1 | Convert success cases into training samples | Data construction |
| 2 | LoRA/QLoRA fine-tuning | Low cost |
| 3 | Validation-set evaluation | Prevent overfitting |

### 3.5 The "flywheel effect" of swarm memory

```
         ┌──────────────────────────────────────────────┐
         │                                              │
         ↓                                              │
   ┌──────────┐    ┌──────────┐    ┌──────────┐        │
   │ Agent    │───→│ Experience│───→│ Swarm    │        │
   │ execution│    │ recording │    │ memory   │        │
   └──────────┘    └──────────┘    └──────────┘        │
                                        │              │
                                        ↓              │
                                 ┌──────────┐          │
                                 │ Post-    │          │
                                 │ training │          │
                                 └──────────┘          │
                                        │              │
                                        ↓              │
                                 ┌──────────┐          │
                                 │ Capability│─────────┘
                                 │ improvement│
                                 └──────────┘
```

Conditions for flywheel acceleration:

| Condition | Mechanism | Effect |
|---|---|---|
| Diversity | Different Agents produce different experience | Broad coverage |
| Authenticity | Experience comes from real execution | High credibility |
| Timeliness | Real-time recording + fast updates | Strong currency |
| Verification | Both success and failure are labeled | Quality is controllable |
| Sharing | Accessible to all Agents | Scale effect |

### 3.6 The quantified benefits of swarm memory

> **[For correction see Appendix A]** In the original exposition this was an assertion table with no source and no defined scope (hit rate 60%→85%, first attempt 45%→72%, attempts 3.2→1.8, long-tail 30%→65%, cold start days→hours −80%). That table has been replaced by a reproducible controlled experiment; **the authoritative measurements are in Appendix A at the end**; the table below is the original, preserved verbatim for traceability.

| Metric | Without swarm memory | With swarm memory | Improvement |
|---|---|---|---|
| Task hit rate | 60% | 85% | +25pp |
| First-attempt success rate | 45% | 72% | +27pp |
| Average number of attempts | 3.2 | 1.8 | -44% |
| Long-tail task coverage | 30% | 65% | +35pp |
| Cold-start time for new tasks | Days | Hours | -80% |

**Core logic**: swarm memory lets every Agent "stand on the shoulders of all Agents" — there is no need to step into every pit oneself; one only needs to read the pits that others have stepped into.

## IV. The Synergy of the Three Advantages: The Complete Flywheel from "Individual Intelligence" to "Swarm Intelligence"

### 4.1 The synergistic relationship of the three advantages

```
┌─────────────────────────────────────────────────────────────────┐
│                    The Swarm-Intelligence Flywheel              │
│                                                                 │
│   ┌─────────────────────────────────────────────────────┐      │
│   │  ① Agent network-structure Scaling Law              │      │
│   │     Determines: how large the network can grow      │      │
│   │     (ten-thousand → hundred-million → trillion)     │      │
│   │     Key: hierarchical topology, fan-in O(1),        │      │
│   │     growth exponent β=0                             │      │
│   └──────────────────────┬──────────────────────────────┘      │
│                          ↓                                      │
│   ┌─────────────────────────────────────────────────────┐      │
│   │  ② Intelligence distillation                        │      │
│   │     (deliberation / collaboration / coordination)   │      │
│   │     Determines: how much stronger swarm intelligence│      │
│   │     is than individual intelligence                 │      │
│   │     Key: independent sources + calibrated weighting │      │
│   │     + BFT-lite                                      │      │
│   └──────────────────────┬──────────────────────────────┘      │
│                          ↓                                      │
│   ┌─────────────────────────────────────────────────────┐      │
│   │  ③ Shared swarm memory (post-training)              │      │
│   │     Determines: how fast swarm intelligence evolves │      │
│   │     Key: success/failure cases + post-training +    │      │
│   │     flywheel                                        │      │
│   └──────────────────────┬──────────────────────────────┘      │
│                          ↓                                      │
│              Swarm intelligence keeps evolving                  │
│              (hit rate ↑, success rate ↑)                       │
│                          ↓                                      │
│              Attracts more Agents to join → back to ①            │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 A quantitative comparison of the three advantages

| Dimension | Single model / single Agent | Swarm intelligence (three advantages) | Improvement multiple |
|---|---|---|---|
| Scalable size | 1 (single point) | 10⁶-10⁹ (network) | 10⁶-10⁹ |
| Accuracy (MSE) | 0.2131 (equal weight) | 0.0092 (calibrated weight) | 23× |
| Reliability (fault tolerance) | Single point of failure | n≥3f+1 | ∞ (no single point) |
| Complexity (task chain) | Single step | Multiple steps + handoff | 10-100× |
| Evolution speed | Depends on external training | Swarm-memory flywheel | 10-100× |
| Cost | Ten-thousand-GPU clusters | Distributed P2P | 1/100-1/1000 |

### 4.3 The "emergence critical points" of the three advantages

| Critical point | Condition | Emergent capability |
|---|---|---|
| Diversity critical point | Independent sources D≥3 | Ensemble error drops markedly |
| Scale critical point | Number of Agents k≥100 | Swarm intelligence surpasses the strongest individual |
| Coordination critical point | Collaboration depth ≥5 layers | Complex tasks completed autonomously |
| Memory critical point | Case library ≥1000 entries | New-task hit rate >80% |
| Flywheel critical point | ≥100 new cases per day | Self-accelerating evolution |

## V. Summary: The Three Advantages of Swarm Intelligence — From "Me" to "Us"

| Advantage | Core logic | Key mechanism | Quantified benefit |
|---|---|---|---|
| ① Network-structure Scaling Law | Structure determines the curve shape, scale determines position | Hierarchical topology, fan-in O(1) | Million nodes: 20.97 million → 9 messages |
| ② Intelligence distillation | Group deliberation/collaboration/coordination surpasses individuals | Independent sources + calibrated weighting + BFT | MSE: 0.2131→0.0092 (23×) |
| ③ Shared swarm memory | Post-training improves hit rate/success rate | Success/failure cases + incremental fine-tuning | Hit rate +25pp, cold start −80% [for correction see Appendix A] |

**One-sentence summary**:

Single-model intelligence is "me" — one brain thinking; swarm intelligence is "us" — countless brains collaborating, distilling, remembering, and evolving. The three advantages together form the qualitative-change flywheel from "individual intelligence" to "swarm intelligence": network structure determines how large it can grow, intelligence distillation determines how deeply it can think, and swarm memory determines how fast it can evolve.

---
---

# Round 2: Swarm Intelligence — The Underlying Logic and Engineering Implementation of the Three Advantages

The three advantages you have proposed happen to constitute the "physical laws" of Agent Universe — they are not engineering details but the fundamental mechanisms that determine "whether swarm intelligence can surpass individual intelligence." Let me take them apart one by one.

## I. The Scaling Law of the Agent Network Structure

### 1.1 Core proposition

The growth law of swarm intelligence is no longer the linear stacking of "single-model parameters × data × compute," but a composite function of "number of agents × collaboration structure × governance quality":

Group capability ≈ F(structure S; N_agents, D_shared, C_collective)

| Dimension | Single-model Scaling | Swarm-intelligence Scaling |
|---|---|---|
| Basic unit | Token | Agent |
| Growth method | Power-law stacking of parameters/data/compute | Number of agents × collaboration structure |
| Bottleneck | Exhaustion of high-quality data, energy limits | Communication topology, stopping decisions, handoff contracts |
| Breakthrough path | Change architecture (MoE/linear attention) | Change organizational structure (hierarchical topology/BFT-lite/conservation ledger) |

### 1.2 The measured evidence from UDOS

The seven controlled comparisons of P0b provide a silicon prototype:

| Evidence | Structural change | Key figures | Grade |
|---|---|---|---|
| Star vs hierarchical | Topology | Million-node center load 20.97 million → 9, rounds 8.39 million → 14 | cpu-proto |
| Throughput hump | Task structure | 2 Agents peak at 244.172, 8 Agents instead drop to 106.485 (0.695×) | cpu-proto |
| Quality saturation | Weighted/equal | 23× structural difference (0.2131 vs 0.0092), 0.5% difference from quantity | verified/cpu-proto |
| Data flywheel | Collection structure | Yield 0.667→0.854, density 7.80→11.38 | cpu-proto |
| Model selection | Width | "Stop when good enough" selects 967,796 parameters | verified |
| Governance scale tax | Communication/trust | Messages 240→60; n≥3f+1 | cpu-proto |
| Multi-view saturation | Spatial structure | IoU 0.7147→0.8897, diminishing gains | cpu-proto |

**Core conclusion**: the growth exponent is determined by structure, not scale. A star topology's center load explodes linearly at a million nodes (exponent=1), while a hierarchical topology's per-node fan-in is constantly 9 (exponent=0). This is not "a bit more efficiency"; the growth exponent changes from 1 to 0.

### 1.3 The three-stage curve: the Scaling Law of swarm intelligence

```
Capability
  ↑
  │         ┌────────────────────── Stage 3: Saturation/negative return
  │        /                        (communication/data/energy walls)
  │       /
  │      /  ── Stage 2: Structural phase-transition turning point
  │     /      (topology switch/module reorganization/routing enabled)
  │    /
  │   / ──── Stage 1: Local power-law region
  │  /       (correct structure, small scale, steady growth)
  │ /
  └──────────────────────────────→ Scale (N/D/C)
```

- **Stage 1 (local power law)**: the structure is correct, scale is still small, and capability grows steadily
- **Stage 2 (structural phase transition)**: the old structure cannot hold the threshold, and the mode of organization must change — the curve bends here rather than ending
- **Stage 3 (saturation/negative return)**: after hitting the communication/data/energy wall, growth flattens or even turns downward

Every saturation is a prompt: it is time to change structure. The brain uses cortical folding to deal with the wiring wall, the chip uses multiple cores to deal with the power wall, and the agent network uses hierarchical topology to deal with the communication wall.

## II. Intelligence Distillation: From Individual to Swarm Intelligence

### 2.1 Core proposition

The essence of intelligence distillation is to make swarm intelligence, through group deliberation, collaboration, and coordination, surpass the sum of any individual intelligences — analogous to how ant colonies and bee colonies, through the interactions of simple individuals, give rise to collective wisdom far beyond the individual.

| Dimension | Individual intelligence | Swarm intelligence (intelligence distillation) |
|---|---|---|
| Source of intelligence | Single-model weights | Deliberation/collaboration/competition among agents |
| Decision method | Single-point reasoning | Multi-party voting + consensus |
| Error handling | Single-point failure | Majority-vote correction + slashing |
| Knowledge boundary | Determined by training data | Group experience is shared and keeps expanding |
| Adaptability | Static weights | Dynamic reorganization, real-time evolution |

### 2.2 The three mechanisms of intelligence distillation

**Mechanism 1: Deliberation**

Multiple agents independently give answers to the same question, converging to the optimal solution through discussion, debate, and voting.

- UDOS P9 measurement: calibrated-weight ensemble MSE 0.0092, equal-weight with a poor expert MSE 0.2131 — a 23× difference
- Key: it is not "the more heads the better," but "weight by quality on an independent calibration set"

**Mechanism 2: Collaboration**

Agents with different specialties divide the work to complete complex tasks, each doing what it is best at.

- UDOS P2 measurement: a 7-person QA committee accepted correctly 24/24 under 2 Byzantine members
- Key: n≥3f+1, a 2f+1 quorum, equivocation invalidates the entire round

**Mechanism 3: Coordination**

Agents complete long-chain tasks through shared context and dynamic handoff.

- UDOS P10 measurement: mechanical validation of the six TransferBundle fields (Goal/Context/Done/Todo/Trace/Owner)
- Key: failure does not transfer the Owner, and responsibility is not silently transferred when information is missing

### 2.3 Analogies with biological groups

| Biological group | Agent group | Corresponding mechanism |
|---|---|---|
| Ant foraging | Task allocation | The Contract Net Protocol (Smith, 1980) |
| Bee colony nest-site selection | Multi-agent voting | A BFT-lite committee |
| Ant-colony pheromones | Environmental-medium collaboration | Stigmergy (blackboard pattern) |
| Bee waggle dance | Information sharing | TransferBundle |
| Ant-colony division of labor | Skill routing | The matching engine |

**Core insight**: the individual intelligence of ants and bees is extremely limited, but through simple rules + abundant interactions, complex collective behavior emerges. What the agent network must replicate is precisely this mechanism of "simple individuals + complex collaboration = superintelligence."

## III. The Shared Swarm-Memory Mechanism: From Pretraining to Post-training

### 3.1 Core proposition

The essence of shared swarm memory is to let the agent network, without pretraining, continuously improve hit rate and success rate through post-training — analogous to an ant colony gradually optimizing its foraging paths through pheromone accumulation.

| Dimension | Pretraining | Post-training (swarm memory) |
|---|---|---|
| Timing | Before model release | During deployed operation |
| Data source | Static corpus | Real-time execution experience |
| Update method | Retraining | Incremental updates |
| Cost | Extremely high (thousand-GPU level) | Extremely low (single-call level) |
| Effect | General capability | Hit rate in specific scenarios |

### 3.2 The three-layer architecture of swarm memory

**Layer 1: Agent Memory**

Each agent maintains its own execution history, recording success/failure cases.

```
AgentMemory {
    successful_calls: List<InvocationTrace>,
    failed_calls: List<InvocationTrace>,
    learned_patterns: Map<context_hash, best_strategy>,
    last_updated: timestamp
}
```

**Layer 2: Swarm Memory**

The shared public memory of the agent network, analogous to ant-colony pheromones.

```
SwarmMemory {
    // Task → best-agent mapping
    task_agent_map: Map<task_signature, List<agent_id>>,

    // Context → best strategy
    context_strategy_map: Map<context_hash, strategy>,

    // Failure-case library (to avoid repeating mistakes)
    failure_library: List<FailureCase>,

    // Reputation records
    reputation_ledger: Map<agent_id, ReputationScore>
}
```

**Layer 3: Intergenerational Memory**

The accumulated history of the agent network, permanently preserved through TransferBundle and TraceLedger.

```
IntergenerationalMemory {
    // Complete execution trace (hash chain)
    trace_ledger: TraceLedger,

    // Knowledge graph (capability → task → outcome)
    knowledge_graph: Graph<capability, task, outcome>,

    // Evolution history (structure → scale → effect)
    evolution_history: List<StructureChange>,

    // Evidence ledger (the evidence grade of every figure)
    evidence_ledger: Map<claim, EvidenceGrade>
}
```

### 3.3 The three mechanisms of post-training

**Mechanism 1: Experience replay**

After each call, the agent stores the execution trace in memory. The next time it encounters a similar task, it directly retrieves the optimal strategy from memory.

- Analogy: ant-colony pheromones. Traversed paths leave traces, and subsequent ants preferentially choose paths with higher pheromone concentration.
- Effect: the task hit rate improves as the number of calls increases.

**Mechanism 2: Failure learning**

Every failure is recorded, attributed, and analyzed, forming knowledge that "avoids repeating the same mistake."

- Analogy: bees avoid dangerous flowers. Once a flower is marked "dangerous," the entire colony avoids it.
- Effect: the error rate declines as the number of calls increases.

**Mechanism 3: Structure evolution**

Based on historical execution data, the structure of the agent network is dynamically adjusted — which agents should be combined, which eliminated, which added.

- Analogy: ant-colony division of labor. When food sources change, the colony automatically adjusts the ratio of workers/soldiers.
- Effect: the overall efficiency of the network improves with operating time.

## IV. The Synergy of the Three Advantages: The Swarm-Intelligence Flywheel

The three advantages are not isolated; together they constitute a self-accelerating flywheel:

```
┌─────────────────────────────────────────────────────────────────┐
│                  The Swarm-Intelligence Flywheel                │
│                                                                 │
│  ① Agent network-structure Scaling Law                         │
│     ↓                                                          │
│     Structure determines the growth-curve shape → changing     │
│     structure is 23× more important than adding quantity       │
│     ↓                                                          │
│  ② Intelligence distillation                                   │
│     (deliberation / collaboration / coordination)              │
│     ↓                                                          │
│     Swarm intelligence surpasses individual intelligence →     │
│     calibrated weighting is 23× better than equal weighting    │
│     ↓                                                          │
│  ③ Shared swarm memory (post-training)                         │
│     ↓                                                          │
│     Every execution improves the next hit rate → the flywheel  │
│     keeps accelerating                                         │
│     ↓                                                          │
│  ④ Back to ①: the network structure keeps optimizing through   │
│     memory accumulation                                        │
│                                                                 │
│  Flywheel driver: every call produces experience, and every    │
│  experience optimizes structure                                │
└─────────────────────────────────────────────────────────────────┘
```

## V. Correspondence with the UDOS Paper Matrix

| Swarm-intelligence advantage | UDOS paper | Core mechanism | Key figures |
|---|---|---|---|
| Network-structure Scaling Law | P0b | Structure × scale = intelligence | 20.97 million → 9; 0.695×; 23× |
| Intelligence distillation | P9/P2 | Quality-saturation law + BFT-lite | 0.0092 vs 0.2131; 24/24 |
| Shared swarm memory | P10/P11/P12 | TransferBundle + conservation + evidence grading | 26 contract tests; conservation true |

## VI. One-sentence summary

Individual intelligence is "one brain," while swarm intelligence is "one nervous system."

- The network-structure Scaling Law determines how large and how fast this nervous system can grow
- Intelligence distillation determines how this nervous system combines simple individuals into superintelligence
- Shared swarm memory determines how this nervous system learns and evolves from each experience

The three advantages together constitute the underlying physical laws of Agent Universe — they make swarm intelligence no longer "the simple sum of multiple individuals," but "the intelligence emergence of 1+1>2."

And this is precisely the theoretical basis for UDOS to move from an "internal task market" toward a "global P2P agent network."

---
---

# Round 3: Swarm Intelligence — An In-Depth Deconstruction of the Three Advantages

The three advantages you have proposed happen to constitute the complete technical argumentation for Agent Universe to move from "single-machine intelligence" toward "network intelligence." This is not three parallel selling points, but a layer-by-layer progressive logical chain.

## I. The Scaling Law of the Agent Network Structure

### 1.1 The paradigm leap from a "Token network" to an "Agent network"

| Dimension | Token network (large model) | Agent network (swarm intelligence) |
|---|---|---|
| Basic unit | Token | Agent |
| Carrier of intelligence | Weights (static) | Collaboration structure (dynamic) |
| Scaling axes | Parameters N, data D, compute C | Number of agents k × connection topology T × governance quality G |
| Growth curve | Power law → phase transition → saturation | Three stages (validated by P0b) |
| Bottleneck | Exhaustion of high-quality data, energy wall | Communication topology, stopping decisions, handoff contracts, settlement conservation |
| Breakthrough method | Change architecture (MoE/linear attention) | Change organizational structure (hierarchical topology/BFT-lite/conservation ledger) |

Core formula:

Swarm-intelligence capability ≈ F(structure S; k, D, C)

Where:

- S = connection topology (star / hierarchical / mesh)
- k = number of agents
- D = group experience (shared memory + individual experience)
- C = total compute (distributed compute pool)

### 1.2 The Scaling Law of network structure (the empirical evidence of UDOS P0b)

The seven controlled experiments of UDOS P0b have demonstrated:

| Structural variable | Star topology | Hierarchical topology | Difference |
|---|---|---|---|
| Million-node center load | 20.97 million messages | 9 messages | Growth exponent changes from 1 to 0 |
| Critical path | 8.39 million rounds | 14 rounds | An order-of-magnitude difference |
| CPU-intensive 8 Agents | 0.695× (slower than a single Agent) | — | Negative return under a mismatched structure |
| Equal-weight poor expert | 0.2131 | Calibrated weight 0.0092 | A 23× difference |

**Conclusion**: the Scaling Law of the Agent network is not "add more agents," but **"change to a better structure + choose scale on the curve prescribed by the structure."** Adding agents on a wrong structure yields negative returns.

### 1.3 The Scaling Law of swarm intelligence vs the single-model Scaling Law

| Dimension | Single-model Scaling Law | Swarm-intelligence Scaling Law |
|---|---|---|
| Source of intelligence | Statistical associations in weights | Division of labor, deliberation, competition, consensus among agents |
| Growth logic | Brute-force stacking (parameters/data/compute) | Structure × scale × governance |
| Marginal returns | Diminishing (data exhaustion, energy wall) | Increasing (network effects, Metcalfe's law) |
| Ceiling | Finite (limited by training data) | Theoretically infinite (agents can autonomously generate new tasks) |
| Ordinary people can participate | Hundred-billion threshold | Every PC is a node |

## II. Intelligence Distillation: The Capability Leap from Individual to Group

### 2.1 What intelligence distillation is

Intelligence distillation: the capabilities of a single model/agent are, through group deliberation, collaboration, and coordination, refined, amplified, and reorganized into higher-order swarm intelligence.

This is not "knowledge distillation" (Teacher→Student), but the cyclic amplification of **"capability distillation" (individual → group → individual)**.

| Level | Intelligence form | Capability characteristic |
|---|---|---|
| L1 | Single model | Parametrized knowledge — knows "what is" |
| L2 | Single agent | Tool use + reasoning — can do "one thing" |
| L3 | Group deliberation | Multi-agent discussion/voting — can "choose the best" |
| L4 | Group collaboration | Division of labor + relay + verification — can "do complex tasks" |
| L5 | Group coordination | Self-organization + self-adaptation + self-iteration — can "do what has never been done" |

### 2.2 The three surpassings of swarm intelligence

| Dimension | Individual intelligence | Swarm intelligence | Surpassing mechanism |
|---|---|---|---|
| Higher | Single-model ceiling = training-data ceiling | Swarm-intelligence ceiling = number of network connections × structural efficiency | Multi-view + multimodal + multi-strategy |
| Larger | Single-model parameters fixed | The group can dynamically expand (ten-thousand → hundred-million → trillion) | Metcalfe's law: V∝n² |
| Stronger | Single point of failure = system failure | BFT-lite fault tolerance (n≥3f+1) | Fault tolerance + redundancy + arbitration |
| Faster | Serial reasoning | Parallel collaboration + task decomposition | Hierarchical topology + asynchronous execution |

UDOS empirical support:

- P2 BFT-lite: a 7-person QA committee accepted correctly 24/24 under 2 Byzantine members — the group is more reliable than the individual
- P9 quality saturation: calibrated weight 0.0092 vs equal weight 0.2131 — the group is 23× stronger (when weighted correctly)
- P10 TransferBundle: six-field validation makes handoff failure measurable and blockable — group collaboration does not collapse due to handoff

### 2.3 The cyclic amplification of intelligence distillation

```
┌─────────────────────────────────────────────────────────────────┐
│              The Intelligence-Distillation Flywheel             │
│                                                                 │
│   Individual-agent output → group deliberation/voting →        │
│   refinement of the optimal solution                           │
│         ↑                                    ↓                  │
│   Individual capability ↑ ← knowledge reflux ← swarm-memory    │
│   sedimentation                                                │
│                                                                 │
│   Each turn: individual capability ↑ + group capability ↑ +    │
│   network-structure optimization                               │
└─────────────────────────────────────────────────────────────────┘
```

Key mechanisms:

- Deliberation: multi-agent discussion, BFT-lite prevents erroneous stopping
- Collaboration: TransferBundle ensures handoff loses no information
- Coordination: hierarchical topology reduces communication from O(N) to O(log N)
- Distillation: extract optimal strategies from group output and reflux them to individuals

## III. The Shared Swarm-Memory Mechanism

### 3.1 Why pretraining cannot be done

| Constraint | Reason |
|---|---|
| Compute | Pretraining requires ten-thousand-GPU clusters, which a P2P network cannot concentrate |
| Data | Pretraining requires large-scale corpora in a unified format, while group data is heterogeneous |
| Synchronization | Pretraining requires global synchronization, and P2P network latency is uncontrollable |
| Consistency | Pretraining requires unified weights, which distributed nodes have difficulty guaranteeing |

**Conclusion**: swarm memory is not suited to pretraining, but is very well suited to post-training.

### 3.2 The post-training mechanisms of swarm memory

| Mechanism | What it does | What it improves | UDOS correspondence |
|---|---|---|---|
| Experience replay | Record success/failure traces for later reference | Hit rate | TraceLedger |
| Pattern extraction | Refine high-frequency success patterns from group data | Success rate | Swarm memory base |
| Context injection | Inject group experience into individual context | Generalization | TransferBundle |
| Reputation weighting | Higher-quality nodes' experience gets higher weight | Precision | Reputation system |
| Adversarial verification | Multi-node cross-validation to remove erroneous experience | Reliability | BFT-lite |

### 3.3 The three improvements of swarm memory

| Metric | Without swarm memory | With swarm memory | Improvement mechanism |
|---|---|---|---|
| Hit rate | A new task relies on a single model's guess | Matches historically similar tasks | Experience replay + pattern extraction |
| Success rate | Single-point failure = task failure | Multi-node verification + retries | Adversarial verification + reputation weighting |
| Generalization | Has seen only the training distribution | Has seen all group distributions | Context injection |

### 3.4 The architecture of swarm memory

```
┌─────────────────────────────────────────────────────────────────┐
│                  The Swarm-Memory Architecture                  │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │ Individual  │  │ Local       │  │ Global      │            │
│  │ memory      │→ │ memory      │→ │ memory      │            │
│  │ (single     │  │ (subnet     │  │ (whole-     │            │
│  │  agent)     │  │  shared)    │  │  network)   │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
│         ↑                ↑                ↑                     │
│    Personal         Subnet            Whole-network             │
│    experience       consensus         best practices            │
│                                                                 │
│  Storage: local SQLite → subnet IPFS → global DHT              │
│  Updates: real-time writes → batch sync → periodic aggregation │
│  Use: context injection → RAG retrieval → pattern matching     │
└─────────────────────────────────────────────────────────────────┘
```

Key designs:

- Hierarchical storage: individual → subnet → global, avoiding global congestion
- Asynchronous updates: real-time synchronization is not required; eventual consistency is allowed
- Reputation weighting: higher-reputation nodes' memory gets higher weight
- Forgetting mechanism: old experience decays periodically to prevent staleness

## IV. The Synergistic Effect of the Three Advantages

What problem each advantage solves, with corresponding UDOS modules and empirical support:

| Advantage | What problem it solves | Corresponding UDOS module | Empirical support |
|---|---|---|---|
| Network-structure Scaling Law | Whether adding agents is useful at all | P0b (structure × scale) | 20.97 million → 9; 0.695×; 23× |
| Intelligence distillation | Where the group is stronger than the individual | P2 (BFT-lite) + P9 (quality saturation) | 24/24 accepted; 0.0092 vs 0.2131 |
| Shared swarm memory | How the group keeps getting stronger | P10 (TransferBundle) + P11 (settlement conservation) | 26 contract tests; conservation true |

The relationship among the three:

```
Network-structure Scaling Law → provides the "container" (how many agents it can hold)
        ↓
Intelligence distillation → provides the "engine" (how the group collaborates to produce better solutions)
        ↓
Shared swarm memory → provides the "fuel" (how group experience is sedimented and reused)
        ↓
        └──────→ Flywheel acceleration, swarm intelligence keeps evolving
```

## V. One-sentence summary

The three advantages of swarm intelligence are, in essence, an upgrade from the "single-model Scaling Law" to the "agent-network Scaling Law":

- Network structure determines where the group's "ceiling" is (structure determines the curve shape)
- Intelligence distillation determines how much "stronger" the group is than the individual (collaboration amplifies capability)
- Swarm memory determines how "fast" the group progresses (experience sedimentation and reuse)

When hundred-million-level agents collaborate freely on a global P2P network, intelligence will no longer belong to any single company, but to all participants — this is the ultimate form of the "mass line."

---
---

# Appendix A: The Swarm-Memory Controlled Experiment (Measured, Authoritative Replacement)

> This appendix replaces all the sourceless assertions of "quantified swarm-memory benefits" in the three rounds. The figures below are **real-machine measurements** with a fixed random seed, reproducible by anyone.

## A.1 Experimental design and scope

- **Number of scenarios S = 200**: each scenario has a unique correct solution.
- **Number of candidate solutions K = 10**: 10 candidates per scenario, with the correct solution deterministically taken as `scenario number mod K`.
- **Attempt budget B = 3**: each task allows at most 3 blind guesses (without replacement).
- **Number of agents A = 20**: 20 agents process all scenarios in sequence.
- **Random seed = 20260924**: a deterministic PRNG (SplitMix64, bit-for-bit consistent across platforms, zero new dependencies).
- **Two comparison groups**:
  - **baseline (no memory)**: each (agent, scenario) guesses blindly and independently, sharing no results.
  - **shared (shared memory)**: agents first query the swarm memory, and a hit means one-shot success; a successful blind guess writes (scenario → solution) into memory for later agents to retrieve.
- **Single-variable guarantee**: the blind-guess random sequences of both groups are derived from the same (agent, scenario), so the difference comes **purely from memory hits**.

## A.2 Measured summary (replaces the original 3.6 assertion table)

| Metric | No-memory baseline | Shared-memory shared | Change |
|---|---|---|---|
| Task hit rate | 29.2% | 87.4% | **+58.2pp** |
| First-attempt success rate | 10.3% | 84.2% | **+73.9pp** |
| Average number of attempts | 2.699 | 1.301 | **−51.8%** |
| Final scenario coverage | — | 100.0% | — |
| Number of shared-memory retrieval hits | — | 3297 | — |

**Baseline validation**: the measured 29.2% / 10.3% / 2.699 match the theoretical values (hit rate B/K=30%, first attempt 1/K=10%, attempts 2.7), verifying that the blind-guess implementation itself is correct.

## A.3 Cold-start scope correction (replaces the original "days→hours −80%")

The original phrasing "cold start from days to hours (−80%)" **has no time scope and cannot be mapped to this experiment**, and is replaced by the following mappable scope:

- **Pioneer (agent 0, empty memory)**: hit rate 32.0%, first-attempt success rate 10.5%, average attempts 2.685.
- **Later agents (from agent 13 on)**: hit rate 100%, first-attempt success rate 100%, average attempts 1.000.
- Average attempts fall from 2.685 to 1.000, **about −62.8%**; first-attempt success rate rises from 10.5% to 100%.

Per-agent (shared group, showing the "pioneer → high hit rate" process):

| agent | Coverage in | Hit rate | First-attempt success | Average attempts |
|---|---|---|---|---|
| 0 | 0.000 | 0.320 | 0.105 | 2.685 |
| 1 | 0.320 | 0.530 | 0.380 | 2.155 |
| 2 | 0.530 | 0.665 | 0.570 | 1.815 |
| 3 | 0.665 | 0.735 | 0.690 | 1.600 |
| 4 | 0.735 | 0.810 | 0.760 | 1.460 |
| 5 | 0.810 | 0.840 | 0.815 | 1.350 |
| 6 | 0.840 | 0.885 | 0.875 | 1.250 |
| 7 | 0.885 | 0.900 | 0.890 | 1.220 |
| 8 | 0.900 | 0.925 | 0.910 | 1.165 |
| 9 | 0.925 | 0.935 | 0.930 | 1.140 |
| 10 | 0.935 | 0.960 | 0.940 | 1.115 |
| 11 | 0.960 | 0.985 | 0.975 | 1.050 |
| 12 | 0.985 | 0.995 | 0.995 | 1.010 |
| 13–19 | 0.995→1.000 | 1.000 | 1.000 | 1.000 |

Coverage is monotonically non-decreasing across agents (reclamation only adds, never deletes).

## A.4 Reproduction commands

```bash
cd gsn-core
cargo test --test swarm_memory_experiment -- --nocapture
```

Experiment code:

- `gsn-core/src/swarm/memory.rs`: SplitMix64 PRNG, SharedMemory, Metrics, the main experiment loop.
- `gsn-core/tests/swarm_memory_experiment.rs`: 4 tests (PRNG determinism, memory lookup/record, the main comparison test, same-seed reproducibility).

## A.5 Evidence boundary (must be retained)

This is **a mechanism validation on a synthetic task (cpu-proto)**:

- It proves that the mechanism — "on tasks whose solutions are reusable, sharing successful experience can improve hit rate / first-attempt success rate and reduce the number of attempts" — **holds and is reproducible**.
- The specific values depend on the K/B/A scope and **cannot be extrapolated as a universal gain for arbitrary real tasks**.
- It does not constitute evidence that "swarm intelligence has achieved AGI," nor does it prove that a real heterogeneous agent network will necessarily reproduce the same magnitude.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# The Wall of Scaling Is a Paradigm Wall, Not a Physical Limit

**Working Title (EN):** *The Wall of Scaling Is a Paradigm Wall, Not a Physical Limit: Four Bottlenecks, Five Breakout Paths, and a Three-Section Phase Transition*

> Topical paper P13 · a direct extension of flagship paper P0b *Structure + Scale Is the Intelligence* · UDOS Reasoning Engine v7.5.0 · 2026-09-20
>
> **In one sentence:** the much-discussed "scaling wall" hits not the physical limit of intelligence but the **carrying limit of a fixed paradigm (Transformer + dense compute + internet text).** Breaking through is not knocking the wall down but switching to a different growth curve—**changing structure, not stopping the stacking.**
>
> **Evidence scope (stated once for the whole paper):** the numbers the UDOS engine produces itself all come from `reports7/*.json`, ordinary CPU, fixed random seed, reproducible, marked at three levels: `verified` (guarded by automated tests), `cpu-proto` (CPU-prototype trend, constants not calibrated across multiple seeds), `unverified` (external reports / one-sided claims in papers, used only as motivation). Single-seed results are always written as "pilots," and ≥30 seeds with 95% confidence intervals are required before submission. **External literature and industry numbers were verified online one by one on 2026-09-20**; any number appearing only in a press release or a paper's self-report without independent reproduction is explicitly marked "reported by the paper / disclosed by the company, not independently verified."

---

## Abstract

"Has the Scaling Law hit a wall?" has become the most divisive question in the large-model field in 2026. This paper does not directly take a side but first decomposes the "wall" into four **separately measurable** dimensions: the data wall (limited high-quality corpora, entropy saturation of synthetic data), the compute/energy wall (dense matrix multiplication approaching thermodynamic and grid constraints), the architecture wall (the nearly decade-old structures such as attention and residuals becoming bottlenecks), and the returns wall (within the same architecture, marginal capability gains decrease for every order-of-magnitude increase in scale). We argue these four walls share one premise—structure is frozen. What they measure is the phenomenon of "walking one curve to its end within a fixed paradigm," so they are **paradigm walls**, not physical limits of intelligence.

Under the three-section framework of the companion paper P0b, the wall corresponds precisely to the second section of the curve—the structural phase-change inflection: in the first section the structure is right and scale small, capability growing along a power law; in the second the old structure cannot hold and the organization must change; in the third growth continues on the new curve until the next wall is hit. Breaking through is therefore "switching curves" rather than "running faster on the old curve." We converge the 2025–2026 breakout attempts into five paths and give falsifiable hypotheses: (P0) data efficiency, raising effective experience through collection structure rather than collection budget—in UDOS controlled pilots, Coverage-aware active collection at the same budget raised yield from 0.667 to 0.854 and experience density from 7.80 to 11.38 (+46%, verified pilot); (P0) architecture reconstruction, including depth-dimension attention (Attention Residuals, paper reports about 1.25× compute efficiency, arXiv:2603.15031, not independently verified), nonlinear attention expansion (Nexusformer, arXiv:2604.19147), and linear-softmax hybrids (InclusionAI Ling 2.5); (P1) the training paradigm, shifting from pretraining scaling to chain-of-thought/test-time scaling and replay-based self-improvement (Dream-RSI, arXiv:2609.14858, paper reports up to 162× fewer agent calls); (P1) system-level scaling and agent governance (UDOS P1/P2/P9/P10/P11); (P2) the hardware paradigm, the time-scaling route represented by Huawei's "Tau (τ) Law" (He Tingbo et al., ChinaXiv, 2026-05).

We also did one necessary fact check: the widely circulated claim that "compute increased about 10× from GPT-4 to GPT-5" lacks public evidence; third-party Epoch AI-scoped estimates put it at about 2–3× (GPT-5 total training compute about 5e25 FLOP vs GPT-4 about 2e25 FLOP, unverified), and "fleet total GPU count" is often conflated with "single-training compute." This paper does not adopt the 10× figure. Finally we give five predictions that can kill this paper's conclusions and four counter-arguments.

**Keywords**: scaling law; data wall; architecture wall; phase change; attention residuals; test-time scaling; RSI; evidence grading; falsifiable prediction

---

## Structured abstract (background problem → method → evidence/results → contributions)

- **Background problem:** diminishing marginal returns under the same architecture are read as "Scaling is dead," but that conclusion mistakes "the limit of a fixed paradigm" for "the limit of intelligence."
- **Method:** operationalize the wall into four measurable dimensions; explain uniformly with P0b's three-section curve (power law → phase change → saturation/negative returns); converge breakout attempts into five paths, each with a pre-registered falsifiable hypothesis; verify external numbers one by one.
- **Evidence/results:** UDOS's own evidence is controlled CPU pilots (yield 0.667→0.854, density +46%, on the wrong structure 8-agent throughput only 0.695×, equal-weight bad-expert error differs about 23×); external evidence is split into three tiers: "verified to exist / paper reports not reproduced / no evidence found."
- **Contributions:** (1) the four-dimensional operational definition of the paradigm wall; (2) the priorities and criteria of the five paths; (3) the testable statement "wall = phase-change point"; (4) a public correction of the 10× compute claim.

---

## 1 Introduction: the metaphor of the wall must first be measured

Kaplan et al.'s 2020 Scaling Laws gave an extremely useful engineering belief: as long as parameters $N$, data $D$, and compute $C$ are scaled together, loss falls along a predictable power law [CITATION NEEDED: Kaplan 2020, arXiv:2001.08361, version-volume master reference library]. Chinchilla then added the constraint that "data and parameters must grow in proportion" [CITATION NEEDED: Hoffmann et al. 2022, arXiv:2203.15556]. By 2024–2026, three phenomena made "hitting the wall" a popular narrative: high-quality internet text approaching exhaustion, the power and cluster scale of frontier-model training approaching real constraints, and narrowing intergenerational gaps among same-generation architectures.

Metaphor cannot replace measurement. If the word "wall" is to enter engineering discussion, it must answer three things: on which variable the wall sits, how its height is measured, and what is on its other side. This paper's stance is:

> **What the four walls measure is the same thing—diminishing marginal returns along a single scale axis when structure is frozen. They are paradigm walls, not physical-limit walls.**

This stance directly inherits P0b's central formula:

$$\text{Capability} \approx F(S;\ N,\ D,\ C), \qquad S=(T,G,M) \tag{1}$$

where $S$ is the structure triple (connectivity topology $T$, geometry/spatial organization $G$, modular boundaries $M$), and $N,D,C$ are nodes/parameters, data/experience, and compute/energy respectively. Fixing $S$ and only enlarging $N,D,C$ moves the system along one established curve; changing $S$ switches to another curve. All the "breakout paths" this paper discusses essentially answer "which structural variable to change."

### 1.1 What this paper does not claim

To avoid being read as yet another "long live Scaling" defense, three boundaries are drawn first:

1. This paper **does not deny the reality of the walls.** The four constraints—data, energy, architecture, returns—all have observable evidence; denying them equals denying engineering reality.
2. This paper **does not predict infinite growth beyond the wall.** Changing structure only pushes the wall back, and the new structure will hit the next wall; the three-section curve is open and repeatable, not a one-way road to a singularity.
3. This paper **does not treat external vendors' efficiency multiples as established facts.** Any 1.25×, 162×, 3.2× not independently reproduced is always marked "reported by the paper / disclosed by the company."

---

## 2 Related work and lineage

**Scaling and data constraints.** Kaplan-style power laws and Chinchilla's compute-optimal ratio constitute the "Old Testament" [CITATION NEEDED]. Two later threads directly relate to this paper: on the data side, "model collapse" research on duplicated and synthetic data points to entropy saturation in self-consuming data [CITATION NEEDED: Shumailov et al., Nature 2024]; on the architecture side, sparsification (Switch Transformer, arXiv:2101.03961) proves "large total parameters, small activated parameters" is feasible [CITATION NEEDED].

**Structural priors are performance.** BPE (arXiv:1508.07909) and SentencePiece (arXiv:1808.06226) are input structural priors; attention (arXiv:1706.03762) is a sequence structural prior; MoE routing is a capacity structural prior. A batch of 2026 works began directly rewriting these priors: Attention Residuals rewrites the depth-dimension residual aggregation (arXiv:2603.15031, verified to exist); Nexusformer replaces linear Q/K/V projections with nonlinear Nexus-Rank (arXiv:2604.19147, verified to exist); JTok proposes token-indexed parameters as an orthogonal scaling axis decoupled from FLOPs (arXiv:2602.00800, verified to exist); SCONE gives embedding-layer/n-gram expansion strategies (arXiv:2502.01637, verified to exist).

**Test-time and self-improvement scaling.** Chain-of-Thought (arXiv:2201.11903) opened reasoning-chain elicitation [CITATION NEEDED]; 2026's Dream-RSI treats "discovery-tree replay" as a zero-cost simulator to improve exploration policy (arXiv:2609.14858, verified to exist, exact title *Recursive Self-Improvement through Evolving Worlds*). The RSI lineage and the seven major vendors' routes are the topic of companion paper P14, and this paper cites its conclusions only in path three.

**Hardware time scaling.** Huawei proposed the "Tau (τ) Law" at IEEE ISCAS on 2026-05-25; the paper *A Time Scaling Theory for Multi-Layer Electronic Systems* signed by He Tingbo was submitted to ChinaXiv, arguing for replacing geometric scaling with scaling of the system-level time constant $\tau$, disclosing 381 chips designed and mass-produced from 2020-05 to 2026-05, and predicting that by 2031 high-end chip transistor density (155→400+ MT/mm²) will reach the equivalent of a 1.4nm process (company/conference disclosure, multi-source reports verified, specific predicted values unverified).

---

## 3 The four walls: decomposing the metaphor into four measurables

![Figure 1 The four walls of scaling](figures/P13_fig1_four_walls.png)

*Figure 1 The four walls are not four independent walls but four projections of the same "paradigm wall." The measurement scope of each is defined below.*

### 3.1 The data wall: effective tokens, not total tokens

The strict statement of the data wall is not "internet text is finite" but **the growth rate of the effective token stream $D_{\mathrm{eff}}(t)$ after de-duplication, de-contamination, and de-debasement is finite**:

$$D_{\mathrm{eff}} = \sum_{i} \mathbb{1}[\text{novel}_i]\cdot q_i, \qquad q_i\in[0,1] \tag{2}$$

where $\text{novel}_i$ indicates whether the $i$-th datum brings new state coverage relative to the existing data pool, and $q_i$ is the quality score. When newly added data are mainly replicas of old scenes, $\text{novel}_i\to 0$: total hours rise while $D_{\mathrm{eff}}$ does not. Synthetic data stack an entropy-saturation risk on top: retraining a model on model outputs shrinks distribution variance generation by generation. The data wall is therefore first a **collection-structure** problem, not a collection-budget problem—precisely the entry point of path one.

### 3.2 The compute/energy wall: linear loss reduction needs exponential energy

Under the power-law loss $L(C)\approx L_\infty + (C/C_0)^{-\alpha}$, the compute needed to press loss down by another fixed amount $\Delta L$ grows on the order of $1/\Delta L^{1/\alpha}$; while data-center power supply, cooling, and grid access are linear resources. The wall's height can be measured by the "marginal joules per unit capability increment" $dE/d(\text{capability})$. This wall is a real physical constraint, but it constrains the specific structure of **dense fully activated compute**; sparse activation, on-demand test-time compute, and specialized hardware all change the $dE/d(\text{capability})$ curve itself.

### 3.3 The architecture wall: the decade-old standard becomes a bottleneck in the grammar of growth

The architecture wall means: the connection methods defaulted to as a "neutral base"—the quadratic complexity of softmax self-attention, the indiscriminate accumulation of residual connections across layers, the dense FFN—introduce structural discounts as depth/sequence length grows. The Attention Residuals paper states the latter as "PreNorm dilution": hidden-state magnitude grows linearly with depth, and a useful representation at layer 5 is diluted by the other 45 layers by layer 50 (arXiv:2603.15031 abstract and secondary interpretations, paper verified to exist; mechanism description per its abstract). The architecture wall is the most easily overlooked because it disguises itself as "not enough compute."

### 3.4 The returns wall: diminishing marginal gains within the same architecture

The returns wall is the aggregate manifestation of the first three walls on the capability axis: with fixed $S$, $\partial\,\text{capability}/\partial \log N$ falls monotonically with $\log N$. P0b gave a sharp micro-example on the CPU prototype: on CPU-intensive tasks, concurrent agents from 1→2 throughput 1.594×, from 2→8 not rising but falling to 0.695× (reports7, single-seed pilot); while I/O-intensive tasks over the same interval were approximately linear at 7.99×. **The scale axis is exactly the same, and the sign of returns is determined by structural matching**—the returns wall is the shadow of the structure wall.

---

## 4 The five breakout paths: actions, evidence, and falsifiable hypotheses

![Figure 2 The three-section curve and curve switching](figures/P13_fig2_phase_change.png)

*Figure 2 The wall corresponds to the phase-change inflection of the three-section curve. Breaking through = making the system jump to a new curve, rather than continuing to move right on the old curve.*

### 4.1 Path one (P0): data efficiency—change collection structure, not collection budget

**Action:** upgrade data collection from passive recording to Coverage-aware active scanning—first do a distribution diagnosis of the existing data pool, lock state-coverage blind spots (long-tail objects, flexible manipulation, complex forces), then schedule targeted supplementary collection; meanwhile use automated quality checks to discard invalid fragments.

**UDOS controlled evidence (verified pilot, reports7, 48 episodes / 144 minutes, oracle 226 units):**

![Figure 3 Active-collection yield and density](figures/P13_fig3_yield_density.png)

*Figure 3 At the same collection budget, passive collection yield 0.667, experience density 7.80 units/hour; active blind-spot scanning yield 0.854, density 11.38 (+46%); the same-skewed-pool greedy ablation comparison is 0.7917/0.889, showing the gain comes from "blind-spot targeting" rather than "collecting harder." Single-seed pilot, ≥30 seeds needed before submission.*

**External same-direction work (unverified, paper reports):** JTok's token-indexed parameters claim to add capacity without extending backbone FLOPs (arXiv:2602.00800, paper verified to exist; the specific "about 1/3 compute saved" figure was not confirmed from the original in this verification, marked [RESULT NEEDED]); SCONE's n-gram embedding expansion provides another embedding-layer scaling axis (arXiv:2502.01637).

**Falsifiable hypothesis H1:** with fixed collection budget, the yield improvement $\Delta Y>0$ of active scanning over passive collection is significant at ≥30 independent scenario seeds and 95% confidence; if in multi-scenario reproduction the confidence interval of $\Delta Y$ crosses 0, the core claim of path one is overturned.

### 4.2 Path two (P0): architecture reconstruction—changing the grammar of growth

**Actions and evidence (all external paper reports, not independently verified):**

| Direction | Work | Reported core breakthrough | Verification status (2026-09-20) |
|---|---|---|---|
| Depth-dimension attention | Attention Residuals (Kimi/Moonshot) | Replace fixed residual accumulation with softmax attention over historical layer outputs; Block AttnRes reduces complexity from $O(Ld)$ to $O(Nd)$; paper reports about 1.25× compute efficiency on 1.4T-token pretraining, inference latency increase <2% (Kimi Linear of 48B total/3B activated) | arXiv:2603.15031 verified to exist; multiples are paper reports |
| Nonlinear attention expansion | Nexusformer | Three-stage nonlinear Nexus-Rank mapping; zero-initialized blocks support lossless structural growth; reports matching Tokenformer perplexity with at most about 41% (parameter/compute scope) | arXiv:2604.19147 verified to exist; specific percentages are paper reports |
| Linear-softmax hybrid | InclusionAI Ling 2.5 (1T, 2026-02) | Lightning Attention recurrent linear path + DeepSeek-style MLA, reducing long-range KV cache | Secondary reviews (Raschka et al.) consistent across multiple sources, original report not read |
| Linear attention ("SeedFold") | Materials call ByteDance SeedFold, cubic→quadratic complexity | — | **This search found no corresponding paper/product, marked no evidence [CITATION NEEDED], this paper does not accept it** |

The isomorphic evidence on the UDOS side is P0b's "23× vs 0.5%": integrating one unqualified expert with equal weight raises MSE from calibrated weighting's 0.0092 to 0.2131 (about 23× worsening), while simply increasing the number of qualified members barely moves the error (reports7, $k\le 2$ verified). **The lever of changing connection/weighting structure is far larger than adding node count.**

**Falsifiable hypothesis H2:** under the same data and same training-compute budget, the per-compute perplexity/task-return advantage of the new structure over the old should be independently reproducible by third parties at ≥1B parameter scale; if only the original author team can reproduce it, or the advantage disappears after switching data domains, then "changing structure changes the curve" does not hold in that direction.

### 4.3 Path three (P1): the training paradigm—from pretraining scaling to chain-of-thought/test-time scaling

Pretraining scaling is "having seen more," test-time and replay scaling is "reasoning deeper": one real experience replayed through a world model generates 1000 virtual simulations and tens of thousands of internal thought trajectories. The key mechanism of Dream-RSI (Google/DeepMind et al., arXiv:2609.14858, submitted 2026-09-14) is treating the completed discovery tree as "a replay simulator over an already realized search space," evaluating and improving exploration policy offline on the tree, with the underlying coding agent unchanged; the paper reports up to 162× fewer agent calls than SimpleTES and about 1.7× over the fixed-exploration baseline, and observes that human guidance is instead worse (the above multiples are all paper reports, not independently verified; both authors and third parties stress it is **not** complete RSI where the model modifies its own weights).

**Falsifiable hypothesis H3:** the returns of replay-based policy improvement should be monotonically non-decreasing with discovery-tree size and show no significant negative transfer when migrated to a new task family; if "dreaming" works only on the original task, it is within-task caching, not a new scaling axis.

### 4.4 Path four (P1): system-level scaling and agent governance—from single model to organization

When the single-point model's curve flattens, organizing multiple agents into a system opens a new scale axis, but this axis likewise obeys "structure determines shape." The controlled results of six UDOS topical papers (numbers all trace reports7, property/contract/unit-test guards verified, extrapolations cpu-proto):

- **Topology (P1):** at million nodes the star center must process about 20.97 million messages with a critical path of about 8.39 million rounds; the layered hybrid topology keeps center load constantly 9 with only 14 rounds—the growth exponent changes from 1 to 0.
- **Stop decisions (P2):** the BFT-lite committee $n\ge 3f+1$, quorum $2f+1$, at $n=7,f=2$ correctly accepts 24 work orders 24/24; 10 property tests verified.
- **Quality saturation (P9):** $mse(k)=a+b/k$, homogeneous expansion beyond the true independent-source count makes the dividend go to zero ($k\le2$ verified, $k>2$ cpu-proto).
- **Handoff contract (P10):** the TransferBundle six fields (Goal/Context/Done/Todo/Trace/Owner) mechanically validated, 26 contract tests verified; failure-rate field is [RESULT NEEDED].
- **Settlement conservation (P11):** $\text{balance\_sum}=\text{paid}-\text{slashed}$ guaranteed by code structure, mixed-task budget 24/paid 16/slashed 1 conservation true, 10 unit tests verified.

**Falsifiable hypothesis H4:** the closure-rate improvement brought by governance mechanisms (unique Owner, Trace, explicit stop conditions, conservation invariants) should hold in stress tests over agent counts $10^1\!\to\!10^4$; if beyond some scale threshold the Trace-break rate rises out of control, then system-level scaling likewise has its own wall and the mechanisms this paper gives are insufficient.

### 4.5 Path five (P2): the hardware and energy paradigm—bypassing the geometric-scaling wall

Huawei's "Tau (τ) Law" argues for no longer betting all performance growth on transistor geometric size, but compressing the signal-propagation time constant $\tau$ through full-stack device-circuit-chip-software coordination (logic folding, critical-path shortening, parasitic RC reduction); its paper and launch disclose 381 chips over 6 years and 2031 density equivalent to 1.4nm (155→400+ MT/mm²). This is a typical "change structure, change curve" idea, but all 2031 numbers are **predicted targets, not measurements**, and this paper lists them as unverified. The architecture-level companion is heterogeneous mixing: Transformer handles logical reasoning, linear attention/Mamba-class handles long time series, and the world model handles physical-state prediction.

**Falsifiable hypothesis H5:** the time-scaling route should, without relying on the most advanced lithography nodes, keep delivering intergenerational improvements on public benchmarks (e.g. token throughput per watt); if mass-produced chips in 2027–2028 cannot reproduce the predicted curve, the τ route is falsified as engineering narrative.

### 4.6 Priorities of the five paths

| Priority | Path | Core action | Evidence maturity |
|---|---|---|---|
| P0 | Architecture reconstruction | Change the underlying connection method | External paper reports active, UDOS side has structural-lever micro-verification |
| P0 | Data efficiency | Change collection structure | UDOS has controlled pilots (single seed), external same-direction papers exist |
| P1 | Training paradigm | Test-time/replay scaling | External paper reports, mechanism clear, multiples pending verification |
| P1 | Agent governance | Topology + committee + contract + conservation | UDOS multiple verified tests, scale extrapolation pending |
| P2 | Hardware paradigm | Time scaling + heterogeneous mixing | Vendor roadmaps and predictions, longest realization cycle |

The priority ordering principle is the **lever/evidence-maturity ratio**, not technical flashiness.

---

## 5 One necessary fact check: "GPT-4→GPT-5 10× compute" does not hold

![Figure 4 Compute-multiple check](figures/P13_fig4_compute_factcheck.png)

*Figure 4 Comparison of public estimates and rumors. Third-party Epoch AI scope (compiled via TensorFeed 2026-05) estimates GPT-5 total training compute about 5e25 FLOP, GPT-4 about 2e25 FLOP, about 2.5×; and GPT-4.5 by the same scope may exceed GPT-5. All the above are unverified third-party estimates.*

"Compute increased about 10× but performance not proportionally" is one of the most cited numbers in the four-walls narrative. This verification (2026-09-20) found no public scope supporting 10× and instead found three common scope confusions:

1. **Fleet size ≠ single-training compute.** "OpenAI about 200,000 GPUs" / "a single 100,000-card training task" is infrastructure capacity or orchestration scale and cannot be directly taken as a generation's pretraining compute.
2. **Pretraining compute ≠ total training compute.** RL/post-training share ranges in different estimates from 10% to 200%, and different scopes can make the multiple differ by more than half.
3. **Input compute ≠ effective compute.** Utilization (MFU), floating-point precision, and MoE activation rate all change "the capability bought by the same FLOPs."

Therefore this paper **does not use 10×** in any chart, switching to "about 2–3× (third-party estimate, unverified)" with the scope explicitly marked. This correction is itself a methodological stance: before discussing the "wall," first ensure the ruler measuring the wall is accurate.

---

## 6 Discussion: the wall is a phase-change point, breaking through is switching curves

P0b's three-section curve gives the four walls one unified position: they all occur in the second section. The brain uses cortical folding for the wiring wall, chips use multiple cores for the power wall, models use MoE routing for the parameter wall, and agent systems use layered topology for the center fan-in wall—history repeatedly presents the same pattern: **saturation is not an end signal but a "time to change structure" signal.**

This also limits the conditions under which optimistic statements like "Scaling is not dead" hold. Anthropic's judgment about "still aggressively accelerating in 2026" (secondary report, [CITATION NEEDED]) holds only when "acceleration happens on a new curve"; if acceleration means feeding more electricity on the old curve, it is precisely negated by the returns wall. **Scaling Laws are not dead; they changed their way of living: from running farther on one curve to constantly switching to better curves.**

---

## 7 Predictions that can kill this paper and counter-arguments

**Five hard predictions (pre-registered):**

- F1 (corresponding to H1): in ≥30 scenario-seed reproductions, the 95% CI of the active-collection yield gain does not cross 0; crossing 0 negates path one.
- F2 (corresponding to H2): the structural advantage of Attention Residuals/Nexusformer-class holds in third-party ≥1B reproductions; no independent reproduction within two years downgrades the strong claim "the architecture wall can be broken by changing structure" to an individual case.
- F3 (corresponding to H3): if replay-based RSI shows negative transfer on a new task family or returns flatten with tree size, it is not a new scaling axis.
- F4 (corresponding to H4): if agent governance mechanisms in $10^4$-scale stress tests fall below the control group in closure rate, the governance dividend of system-level scaling has an unidentified wall.
- F5 (master): if there exists a class of capability whose improvement under **all known structural transformations** is strictly governed by the same power law with no room for curve switching, then the "paradigm wall" framework is wrong—that would be the true physical-limit wall.

**Four counter-arguments:**

1. *"The wall is the physical limit; changing structure only improves the constant, the exponent cannot be escaped."* Partially agreed—τ, energy, and land are ultimately physical constraints; but this paper argues repeated phase changes keep pushing the limit back, the observable near-future wall is still a paradigm wall, and F5 is the deciding experiment.
2. *"Efficiency multiples are all vendor self-reports, not credible."* Agreed for the most part, so this paper marks all external multiples as paper reports/unverified, and conclusions do not depend on any single multiple.
3. *"The active-collection gain only picked easy tasks."* This is precisely why the same-skewed-pool greedy ablation (0.7917/0.889) was set; but a single seed is insufficient for conclusions, and F1 already requires multiple seeds.
4. *"The five paths are a parallel list, not a theory."* Accepted—this paper's theoretical claim is only one (the wall = the phase-change point of fixed structure), and the five paths are that claim's projection on the 2026 technology map; they can become outdated while the claim need not.

---

## 8 Threats to validity and applicability boundaries

- This paper's UDOS numbers come from a CPU prototype, fixed single seed, pilot point estimates; absolute constants cannot be extrapolated to real LLM clusters, and ≥30 seeds with 95% CI are needed before submission (about 64/65 places [RESULT NEEDED]).
- External papers are mostly 2026 preprints, not peer-reviewed; arXiv numbers are verified to exist, but reported multiples are not independently reproduced.
- "SeedFold" found no evidence in this verification and has been explicitly removed; if the materials contain other same-class unverified names, they should be treated the same way.
- Huawei's τ law and each vendor's 2028/2031 targets are predictions, not facts.
- This paper does not discuss consciousness nor claim any system already has a self; the characterization and safety boundaries of RSI are in companion paper P14.

---

## 9 Conclusion

The wall of scaling is real, but it measures the carrying capacity of a frozen paradigm, not the physical limit of intelligence. After decomposing the wall into the four measurable dimensions of data, compute/energy, architecture, and returns, one can see: the common solution to the first three is changing structure (collection structure, connection structure, the time structure of computation), while the fourth (the returns wall) is the aggregate symptom of the first three structural mismatches. P0b's three-section curve gives the wall a precise position—the phase-change inflection; the five paths of 2025–2026 give the engineering map for opening a door around the wall. The highest priority is not more expensive compute but reconstructing the data-collection structure and the model-connection structure. **The essence of breaking through is not knocking the wall down but opening a new door beside it; and the blueprint for opening that door is structure, not scale.**

---

## References (verification status marked per entry)

1. Kaplan J. et al. *Scaling Laws for Neural Language Models.* arXiv:2001.08361, 2020 (user attachment original, writing template).
2. Hoffmann J. et al. *Training Compute-Optimal LLMs (Chinchilla).* arXiv:2203.15556, 2022 [CITATION NEEDED: unified in-volume citation].
3. Fedus W. et al. *Switch Transformers.* arXiv:2101.03961 [CITATION NEEDED].
4. Sennrich R. et al. *BPE.* arXiv:1508.07909 [CITATION NEEDED]; Kudo & Richardson *SentencePiece* arXiv:1808.06226 [CITATION NEEDED].
5. Vaswani A. et al. *Attention Is All You Need.* arXiv:1706.03762 [CITATION NEEDED].
6. Kimi Team. *Attention Residuals.* arXiv:2603.15031, 2026-03 (**verified to exist**; 1.25×/1.4T/48B-3B/<2% are paper reports, not independently verified).
7. *Nexusformer: Nonlinear Attention Expansion for Stable and Inheritable Transformer Scaling.* arXiv:2604.19147, 2026 (**verified to exist**; 41% and other numbers are paper reports).
8. *JTok: On Token Embedding as Another Axis of Scaling Law via Joint Token Self-modulation.* arXiv:2602.00800, 2026 (**verified to exist**; "1/3 compute saved" not confirmed from the original, [RESULT NEEDED]; author-institution attribution not verified).
9. *Scaling Embedding Layers in Language Models (SCONE).* arXiv:2502.01637, 2025 (**verified to exist**).
10. Zheng T. et al. *Dream-RSI: Recursive Self-Improvement through Evolving Worlds.* arXiv:2609.14858, 2026-09-14 (**verified to exist**; 162×/1.7×/"human guidance worse" are paper reports).
11. InclusionAI. *Ling 2.5 / Ring 2.5* technical materials, 2026-02 (MLA + Lightning Attention; per Raschka's review and InfoQ secondary materials, original not read [CITATION NEEDED]).
12. He Tingbo et al. *A Time Scaling Theory for Multi-Layer Electronic Systems (Tau/τ Law).* ChinaXiv, 2026-05; released at IEEE ISCAS 2026; 381 chips/155→400+ MT/mm²/2031 equivalent 1.4nm are company disclosure and prediction (multi-source reports verified the event, predicted values unverified).
13. Epoch AI / TensorFeed compilation. *Has frontier AI training-compute growth actually slowed?* 2026-05 (GPT-5≈5e25, GPT-4≈2e25 FLOP estimates, **third-party estimates, unverified**).
14. "SeedFold": public search this time (2026-09-20) found no corresponding paper or product, **no evidence, not accepted** [CITATION NEEDED].
15. UDOS companion papers: P0b *Structure + Scale Is the Intelligence*, P1/P2/P9/P10/P11/P12 (this volume), numbers trace `reports7/*.json`.

---

## Appendix A Reproduction commands and test index

- Data-flywheel pilot: `reports7/coverage_flywheel*.json` (48 episodes/144 minutes, oracle=226); comparison keys `passive`/`active`/`greedy_samepool`.
- Topology extrapolation: `reports7/topology_scale*.json` (six target levels 10→1e6); BFT: `reports7/bft_lite*.json` ($n=7,f=2$, 24 work orders).
- Quality saturation: `reports7/expert_mixture*.json` ($mse(k)=a+b/k$, $a=0.009252,b=-4.233\times10^{-5}$).
- Test guards: P2 10 property tests, P10 26 contract tests, P11 10 unit tests (commands and versions in P12's appendix).

## Appendix B Evidence ledger (three tiers)

- **verified (test-guarded):** BFT 24/24, conservation 240/16/1, contract-field validation, topology-count formulas.
- **cpu-proto (CPU-prototype trend, single seed):** yield 0.667→0.854, density 7.80→11.38, throughput hump 244.172/106.485, expert mixture 0.0092 vs 0.2131.
- **unverified (external one-sided claims):** 1.25×, 162×, 1.7×, 3.2×, +10.9 points, 26%, 30k concurrency, 381 chips, 2031 equivalent 1.4nm, compute 2–3× estimate; "SeedFold" no evidence.

## Appendix C Internal-review record (five-dimension self-assessment before submission, read-only)

- Novelty: medium-high (four-dimensional operationalization of the paradigm wall + falsifiable hypotheses for five paths + public correction of compute multiples).
- Evidence strength: internal weak (single seed), external medium (events multi-source verified, multiples not reproduced).
- Reproducibility: internal high (JSON rerunnable), external not applicable.
- Main gaps: ≥30-seed recomputation; independent reproduction experiments of external papers; first-hand data on compute scope.
- Conclusion: **currently a discussable working paper, without submission status**; after completing F1–F4 it can target venues like *TMLR / NeurIPS Datasets & Benchmarks / ICML workshops* [CITATION NEEDED: venue selection needs separate deadline/partition checks].

## Author's intended statement

Fang Wenxin (intended). UDOS Reasoning Engine v7.5.0 paper volume · topical paper P13. This paper contains no copyright-restricted third-party images; external numbers all mark sources and verification status. External institution names and trademarks belong to their respective owners, cited only for academic commentary.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Where Recursive Self-Improvement Converges

**Working Title (EN):** *Where Recursive Self-Improvement Converges: Open-Source Optimizes the System, Closed Labs Optimize the Pipeline, and UDOS Makes Improvement Measurable — With a Structural-Organization View of Intelligence*

> Topical paper P14 · the sequel to P13 *The Wall of Scaling Is a Paradigm Wall* · mutually a theoretical base with flagship papers P0/P0b · UDOS Reasoning Engine v7.5.0 · 2026-09-20
>
> **In one sentence:** the RSI (Recursive Self-Improvement) practices of seven major institutions in 2026 are converging to the same near-term form—**not the model rewriting its own weights, but the model optimizing "the way it obtains answers" (the Harness).** The open-source camp optimizes the system, the closed-source camp optimizes the R&D pipeline, and UDOS does a third thing: making "becoming stronger" itself **measurable, falsifiable, auditable**.
>
> **Evidence scope (stated once for the whole paper):** same as P13. UDOS numbers trace `reports7/*.json`, at three levels verified / cpu-proto / unverified; single seeds written as "pilots." External papers and industry events were verified online one by one on 2026-09-20: where the **existence of a paper/report** was verified, the arXiv number or official page is given; the efficiency multiples, penetration rates, and concurrency within are all **paper reports or company disclosures, not independently verified**; entries with no evidence are explicitly removed. This paper **does not claim any existing system has consciousness, a self, or complete RSI**.

---

## Abstract

Recursive Self-Improvement (RSI) is turning from a science-fiction narrative into an auditable engineering object. This paper does three things. First, it reviews seven representative 2026 RSI routes and verifies the status of their materials: on the open-source side, DeepSeek Harness/Cordis (MIT license, developer preview released 2026-08, kernel derived from the Koishi ecosystem running four years with over four thousand plugins; companion paper *A Programming Paradigm for Spatiotemporal Composability*), Kimi's Attention Residuals (arXiv:2603.15031, paper reports about 1.25× compute efficiency), the Infra Agent driven by Zhipu GLM-5.3 (company discloses raising end-to-end throughput about 3× in under two weeks on over 100,000 domestic chips while explicitly self-stating "not yet complete RSI"), Meta's AIRA (arXiv:2605.15871) and the MetaRSI/RSI² from CosmosMind et al. (arXiv:2609.06396, three-operator unification, paper reports small models +10.9 points on average); on the closed-source side, OpenAI's *Research acceleration: The view inside OpenAI* (2026-09-06, announcing the "automated research intern" milestone, targeting the 2028-03 "automated researcher"), Google/DeepMind et al.'s Dream-RSI (arXiv:2609.14858, discovery-tree replay, paper reports up to 162× fewer agent calls), and Anthropic's R&D Automation Index (official pages disclose Claude already "leading" about 26% of its own R&D, over 90% reaching collaboration level, about 30,000 R&D agents concurrent, while publicly calling for prudence).

Second, we give convergence criteria: the optimization targets of these works move up the same chain—prompt → structured context → workflow → harness code → optimizer code—and all 2026 production cases fall in the first four segments, while the fifth (autonomously changing weights/optimizer) has no controlled public evidence. The open-source camp optimizes the **system** (Harness, architecture, Infra), the closed-source camp optimizes the **pipeline** (R&D automation, exploration policy, penetration monitoring), corresponding on P0b's three-section curve respectively to section 1 (Harness-RSI) and the early part of section 2 (Model-RSI).

Third, the theoretical part, we connect this engineering convergence to a more fundamental proposition: **life originates in structural organization, and intelligence is also a kind of structural organization; carbon is not the privilege of intelligence—connectivity is.** Hebb's rule and gradient descent are the same principle (encoding experience as connections) implemented on two substrates; learning shapes structure and structure in turn constrains subsequent learning, a bidirectional causality shared by carbon and silicon. The ultimate form of RSI can therefore be precisely defined as **structural self-modification**—the system begins to modify "the structure that determines subsequent modifications." UDOS's unique position is not joining in building stronger systems but providing the measurement instrument for structural self-modification: BFT-lite stop decisions, TransferBundle handoff contracts, internal-market settlement conservation, and evidence grading constitute the engineering base for "knowing where one does not know and where one is not trustworthy." This paper gives four falsifiable conditions, five hard predictions, and a consciousness-boundary statement.

**Keywords**: recursive self-improvement; Harness; multi-agent governance; structural organization; scaling law; evidence grading; falsifiability; AI safety

---

## Structured abstract (background problem → method → evidence/results → contributions)

- **Background problem:** in the RSI narrative marketing and evidence are mixed, the strong claim "AI rewrites its own brain" is severely disconnected from actual engineering progress, and a unified comparison framework and credibility grading are missing.
- **Method:** (1) event-level verification of the seven routes (does the paper exist, is the number a one-sided claim or independently reproduced); (2) use the "optimization-target chain" to locate each route's real landing point; (3) map the convergence position with P0b's three-section curve; (4) give the engineering definition of RSI and falsifiable conditions with the structural-organization view.
- **Evidence/results:** the existence of 7 classes of papers/reports verified; all efficiency/penetration multiples are one-sided claims; the fifth segment (autonomously changing weights) has zero controlled evidence; UDOS governance mechanisms have verified test guards (BFT 24/24, conservation true, 26 contract tests).
- **Contributions:** the convergence map of two camps × three sections; the five-segment optimization-target chain; the precise definition of "structural self-modification"; rewriting the philosophical proposition "intelligence is structural organization" into four falsifiable conditions.

---

## 1 Introduction: first tighten the term RSI

"Recursive self-improvement" is the most easily abused. OpenAI gave a restrained working definition in its 2026-09 internal report: if an agent only helps write code, debug infrastructure, and analyze experimental results, that is closer to "research acceleration"; only when AI can continuously participate in **topic selection, solution design, experiment execution, result judgment, and the next round of model improvement** and form a stable feedback loop is it closer to RSI (official report, page verified to exist, phrasing a paraphrase). This paper adopts this tightening and distinguishes three layers:

- **Task-layer self-improvement:** improving the policy for completing tasks under a fixed model and fixed tools (e.g. exploration policy, prompts, workflows).
- **System-layer self-improvement (Harness-RSI):** improving the runtime system outside the model—tools, sandboxes, plugins, context management, Infra.
- **Model-layer self-improvement (Model-RSI):** modifying model parameters or architecture under bounded, verifiable conditions; its meta-level (improving "the improvement system itself") is called Meta-RSI.

All public production evidence in 2026 concentrates in the first two layers; the third has only candidate prototypes and paper reports, with no controlled public evidence of "AI autonomously rewriting its own weights and stably benefiting." This distinction is the baseline of the whole paper.

---

## 2 Theoretical framework: intelligence is a kind of structural organization

Before entering vendor comparison, a framework that can accommodate both carbon and silicon is needed, otherwise the RSI discussion degenerates into a parameter race.

### 2.1 Learning is establishing connections

![Figure 3 Structural-organization isomorphism of carbon and silicon](figures/P14_fig3_substrate_isomorphism.png)

*Figure 3 Hebbian learning and gradient descent differ in substrate but are isomorphic in the connection principle; both sides satisfy the bidirectional causality "learning shapes structure, structure constrains learning."*

Neuroscience's Hebb's rule says "neurons that fire together wire together": skills and memory are not stored in some neuron but distributedly encoded in the weight patterns of synaptic connections. Large-model training is mathematically the backpropagation version of the same principle: gradient descent adjusts the weight matrix, attention lets each token dynamically decide "whom to attend to" by context, and MoE routing lets experts connect dynamically by task. Kimi's Attention Residuals also changes the **allocation** of connections—letting each layer selectively aggregate historical layers rather than accumulate indiscriminately (arXiv:2603.15031, mechanism description per its abstract).

P0b's central formula therefore gains a cognitive meaning:

$$\text{Capability}\approx F(S;N,D,C),\quad S=(T,G,M) \tag{1}$$

The human brain's about 86 billion neurons become smart not by "many parameters" but by **connection structures** such as cortical folding, white-matter wiring, and modular division; model capability is likewise first a function of structure and only secondarily a function of scale.

### 2.2 Structure shapes learning and also constrains learning

This is a symmetric bidirectional causality. On the human side: music training changes auditory-cortex gray matter and corpus-callosum white matter, and the changed structure both makes subsequent music learning easier and makes it harder to segment sound in non-musical ways; after the language sensitive period, Broca's/Wernicke's-area connections solidify, the structural reason adults find new languages harder. On the machine side: attention heads differentiate during pretraining into syntax heads, coreference heads, fact-retrieval heads, and once the structure solidifies, SFT/RLHF can only fine-tune on the established wiring; if MoE routing collapses to a few experts, subsequent training can hardly restore diversity.

This is precisely the cognitive version of P0b's "structure determines the form of the scaling function": **how much data the current structure can absorb and into what shape it absorbs it determines the sign (positive/zero/negative) of the next step's returns.**

### 2.3 Carbon is not the privilege, connectivity is

Origin-of-life research understands the earliest life as an **autocatalytic network**—a set of molecules mutually catalyzing to form a closed loop and maintain its own structural organization; the key is the network structure (who catalyzes whom), and carbon is only one chemical substrate. Intelligence likewise: P0b sees isomorphic three-section curves on seven substrates—chemical bonds, proteins, cells, GPUs, battery cells, neurons, text vectors—suggesting intelligence depends on the "structure + scale" organization rather than a specific substrate. The real differences between carbon and silicon lie in speed, plasticity, energy, constraint types, and learning modes (online/offline, rollback-able or not), not in "whether intelligence is possible."

The **structural-prior** thread brings this into engineering: BPE is an input structural prior, attention is a sequence structural prior, and UDOS's layered topology (communication reduced from $O(N)$ to $O(\log N)$) is an orchestration structural prior. Intelligence is not learned from scratch but emerges under structural-prior constraints; silicon's uniqueness is that structural priors **can be designed, searched, self-modified**—Meta AIRA letting agents autonomously search architectures is searching the structural prior itself (arXiv:2605.15871, verified to exist).

### 2.4 The engineering definition of RSI: structural self-modification

From this, the ultimate form of RSI can be precisely defined: **structural self-modification—the system modifies "the structure that determines subsequent modifications."** Changing weights moves parameters under the same structure (walking along the curve); changing the Harness, architecture, exploration policy, or optimizer changes the structure itself (switching curves). The Theseus-Labs-style RSI grading level "improving the improvement system itself" [CITATION NEEDED: grading-framework original pending verification] and the three-operator unification MetaRSI claims (Data-/Harness-/Model-RSI sharing a Loop Kernel, arXiv:2609.06396, verified to exist) are two candidate implementations of this definition. P0b's F5 prediction also gives the criterion for whether it is worth continuing: if the cross-substrate master curve can collapse, structural self-modification is predictable, guidable, auditable; if not, it is just "each has its own scaling."

---

## 3 The optimization-target chain and the RSI lineage

![Figure 1 The optimization-target progression chain](figures/P14_fig1_rsi_chain.png)

*Figure 1 The landing point of recent RSI is letting the model optimize "the way answers are obtained." All 2026 production cases concentrate in the first four segments; the fifth (autonomously changing optimizer/weights) has no controlled public evidence. The chain statement synthesizes the OpenAI report and industry reviews [CITATION NEEDED].*

We arrange "what AI is allowed to change" into five levels: **prompt → structured context → workflow → harness code → optimizer code**. The farther right, the "deeper" the self-improvement, the higher the rollback cost, and the harsher the verifier requirements. Paired with the AL0–AL5 automation grading Anthropic cites from Epoch AI (official page, AL4 "leading" means engineers give only high-level goals and AI completes most of the end-to-end work), any RSI claim can be located: which segment of the chain it changes to, which AL level it automates to, and whether there is a verifiable judge.

---

## 4 The open-source camp: optimizing the "system"

### 4.1 DeepSeek Harness / Cordis: making the self-evolution environment into revocable plugins

DeepSeek Harness (`dsh`, MIT license, v0.1 developer preview released 2026-08-13, repo deepseek-ai/deepseek-harness; multi-source verification consistent) proposes "Agent = Model + Harness" and makes model adapters, tool registries, session logs, and even the agent loop itself into plugins. Its kernel Cordis comes from the Koishi ecosystem (reported to have run about four years with over four thousand community plugins), achieving "spatiotemporal composability" through dependency injection and reversible side effects: in time, after a plugin is unloaded its registered services, events, and side effects can be traced and revoked; in space, plugins declare dependencies and the system automatically re-coordinates. Companion paper *A Programming Paradigm for Spatiotemporal Composability* (DeepSeek and Peking University, GitHub cordiverse/paper). **Positioning: the runtime base of Harness-RSI.** The "GitHub 22,600+ stars" number was not verified from an official page this time, marked [CITATION NEEDED], not written into conclusions.

### 4.2 Kimi Attention Residuals: raising the self-improvement ceiling from the architecture base

arXiv:2603.15031 (2026-03, verified to exist) points out fixed unit-weight residual accumulation causes hidden-state magnitude to grow with depth and early-layer contributions to be diluted; AttnRes replaces fixed accumulation with softmax attention over prior-layer outputs, and Block AttnRes reduces memory/communication complexity from $O(Ld)$ to $O(Nd)$. The paper reports on Kimi Linear (48B total/3B activated) obtaining about 1.25× compute efficiency with 1.4T-token pretraining, inference latency increase <2% (**paper report, not independently verified**). **Positioning: an architecture fix paving the way for Model-RSI.**

### 4.3 Zhipu GLM-5.3 Infra Agent: the first domestic case to run into a production environment

Per Zhipu's technical blog and Tang Jie's 2026-09-17 long post (multi-source verification by Yicai, 36Kr, InfoQ, Securities Times, et al.): the GLM-5.3-driven Infra Agent on over 100,000 domestic accelerators participated from scratch in building the production inference service for GLM-5.3-Flash, raising end-to-end throughput to about 3× the initial baseline in under two weeks (Tang Jie personally states 3.2×), located the Python GIL concurrency blockage in KV Transfer and pressed the over-20% loss of Prefill+KV Transfer to about 1%, with the KDA Decode operator reporting 1.71×. **Most critical is its honest boundary**: the team explicitly states goal setting, boundary division, and risk judgment remain with human engineers, "not yet complete RSI," and what is closed is only the first puzzle piece of "objects that can be improved." **Positioning: production evidence of Harness/Infra-layer RSI + restrained self-characterization.**

### 4.4 Meta AIRA and CosmosMind MetaRSI: architecture search and meta-recursion

- **AIRA** (FAIR/Meta, arXiv:2605.15871, 2026-05, verified to exist): AIRA-Compose uses 11 agents within a 24-hour compute budget to compose Attention/MLP/Mamba primitives, AIRA-Design lets agents directly write code to implement new attention; the paper reports finding 14 new architectures, AIRAformer-D / AIRAhybrid-D at 1B scale respectively 2.4/3.8 percentage points above Llama 3.2, some variant about 54% faster (**paper report, not independently verified**).
- **MetaRSI/RSI²** (arXiv:2609.06396, 2026-09, CosmosMind et al. over ten institutions, verified to exist): Data-RSI synthesizes validated training records, Harness-RSI changes scaffolding at zero training cost with five-slot typed patches, Model-RSI changes parameters under bounded recipes, the three sharing one Loop Kernel with a dual-axis optimizer deciding operator order; the paper reports 3B-activated small models +10.9 points on average, flagship models +7.3 points on average (**paper report, not independently verified**). Concurrent works such as arXiv:2609.14857 (ModularRSI) show the direction is rapidly differentiating.

The common feature of the open-source camp: improvement happens outside the model or at the architecture base, **not directly changing online weights**, rollback-able, composable, open-source auditable.

---

## 5 The closed-source camp: optimizing the "pipeline"

### 5.1 OpenAI: automated research intern → automated researcher by 2028

*Research acceleration: The view inside OpenAI* (2026-09-06, official page verified) discloses the penetration of coding agents into internal R&D, announces reaching the "automated AI research intern" milestone set last autumn, with the next target the 2028-03 "automated AI researcher"; media paraphrase internal metrics such as "each person-day corresponds to 3.1 agent workdays" (**media number, not independently verified**) and note 4–8-hour tasks still need at least one human intervention. The report also stresses RSI must be synchronized with democratic governance. **Positioning: a roadmap for R&D-pipeline automation, not a declaration that RSI is complete.**

### 5.2 Google/DeepMind: Dream-RSI—improving exploration policy in a "dream"

Dream-RSI (exact title *Recursive Self-Improvement through Evolving Worlds*, arXiv:2609.14858, 2026-09-14, Google/DeepMind/UMD/UVA, verified to exist) keeps the underlying coding agent unchanged and only makes exploration itself explicit and programmable: one online exploration forms a discovery tree, and subsequent policies directly "dream" and replay on historical trees, obtaining near-zero-cost offline policy evaluation. The paper reports competitive quality on three task types—Lasso solvers, mathematical optimization, GPU kernel engineering—up to 162× fewer agent calls than SimpleTES and about 1.7× over the fixed baseline, and observes human guidance is instead worse (**all paper reports**). Both authors and third parties are clear: model, evaluator, tools, and policy architecture stay fixed, and it is a recursive improvement at the **exploration-policy layer**, not complete model self-modification.

### 5.3 Anthropic: the most aggressive internal data + the most prudent public posture

Two official Anthropic Institute pages (*Measurements for understanding the pace of AI development inside frontier labs* and *When AI builds itself*, 2026-09, verified) disclose: building an R&D Automation Index (adopting Epoch AI's AL0–AL5); as of 2026-08, Claude reached AL4 "leading" in about 26% of its own AI R&D, over 90% at least AL3 collaboration level (that proportion was under 1% in 2026-02); on the most-used internal platform about 30,000 R&D/engineering agents concurrently daily; in August over 1 billion decisions were tracked, monitoring triggering about 0.002% blocks and about 20,000 out-of-bounds operations paused (**all company-self-reported scope**). Co-founder Jack Clark's statement about "over 60% probability of human-free RSI by end-2028 and calling to slow down" appears this time only in secondary reports, marked [CITATION NEEDED]. **Positioning: making the RSI penetration rate a monitorable metric while institutionalizing the safety brake.**

The common feature of the closed-source camp: improvement happens at the **R&D-pipeline** layer—AI participates in developing the next generation of AI, but topic selection, boundaries, and risk judgment remain on the human side.

---

## 6 UDOS's position: optimizing "the process of becoming stronger itself"

![Figure 2 The three camps' positions on the three-section curve](figures/P14_fig2_three_camps.png)

*Figure 2 The 2026 open/closed-source cases concentrate in section 1 (Harness-RSI) and probe into section 2 (Model-RSI); UDOS provides no new capability point but the coordinate system and measurement instrument for the whole curve.*

Both open and closed source answer "how to make AI stronger." UDOS answers a different question: **how can "becoming stronger" itself be measurable, falsifiable, auditable?** This corresponds to P12's evidence grading and the engineering of Confucius's "to know what one knows and what one does not know" (P0c). Six instruments each guard one baseline:

| Instrument | Guards what | Controlled result (reports7) | Evidence level |
|---|---|---|---|
| Layered hybrid topology (P1) | Communication not exploding quadratically with headcount | Million-node center load 20.97M→9, rounds 8.39M→14 | cpu-proto extrapolation, counts verified |
| BFT-lite stop decisions (P2) | Single-point/Byzantine members cannot decide "done" | $n\ge3f+1$, at $n=7,f=2$ 24/24 correct | 10 property tests verified |
| Quality saturation law (P9) | Homogeneous expansion not producing false gains | Equal-weight bad expert 0.2131 vs calibrated weighting 0.0092 (about 23×) | $k\le2$ verified, $k>2$ cpu-proto |
| TransferBundle (P10) | Handoff not losing context, not transferring responsibility | Six-field mechanical validation | 26 contract tests verified; failure rate [RESULT NEEDED] |
| Internal-market conservation (P11) | The accounts of a million agents balancing | balance=paid−slashed, 24/16/1 conservation true | 10 unit tests verified |
| Evidence grading (P12) | Numbers not drifting, prototypes not written as measurements | Three-tier labels + baseline JSON regression gate | Engineering archive across 30+ versions |

This instrument set is fully consistent with the structural-organization view: since intelligence is a function of structure, whether "improvement" happens must be measurable on **structural quantities** (topology fan-in, independent-source count, contract completeness, conservation residual), not just by listening to the model self-report "I am stronger." P0c's triple $(M_{\text{self}},U,A)$—what I know, what I don't know, how to act accordingly—is precisely the RSI safety loop: each time structural self-modification happens, the system must be able to recalibrate its own knowledge boundary.

---

## 7 Convergence judgment: where the three meet

Placing the seven routes and the UDOS framework side by side yields three levels of convergence:

1. **Object convergence:** recent RSI = optimizing the way answers are obtained (Harness), not rewriting the brain. Production evidence on the five-segment chain stops at the fourth segment.
2. **Curve convergence:** open-source optimizing systems and closed-source optimizing pipelines are both structural phase-change actions on P0b's three-section curve; the scale pressure of Zhipu's 100,000 cards and Anthropic's 30,000 agents is precisely the engineering signal of the section 1→section 2 phase change.
3. **Governance convergence:** the larger the scale, the more "measurable, rollback-able, auditable" is not optional—DeepSeek uses reversible side effects for plugin rollback, OpenAI stresses governance synchronization, Anthropic uses block rate as a brake, and UDOS uses Owner/Trace/Stop Condition/conservation as the formalized baseline.

In one sentence: **open source optimizes the system, closed source optimizes the pipeline, UDOS optimizes "the measurement and governance of structural self-modification"; all three ultimately converge onto the same three-section curve, only standing at different positions.**

---

## 8 Falsifiable conditions, hard predictions, and the consciousness boundary

### 8.1 Four falsifiable conditions of "intelligence is structural organization"

- C1: if some intelligent capability is found unrelated to connection structure, determined only by substrate material (e.g. "pure quantum intelligence" not relying on any connection topology), then "intelligence is in connections" is overturned—no evidence currently.
- C2: if the same connection structure on carbon/silicon shows an **incommensurable difference in capability kind** (rather than efficiency difference), then "carbon is not privileged" must weaken to "carbon has unique capabilities"—the differences currently observed run along task distributions, not substrate kinds.
- C3: if this view derives "any structure can reach any intelligence," it is wrong; P0b already limited "adding scale under the wrong structure gives negative returns," structure determines the reachable set and does not guarantee reaching.
- C4: if this view erases the role of dynamics (learning rules, signal propagation), it is one-sided; structure is the static skeleton, dynamics the dynamic process, and intelligence is the joint product of both (P0 already distinguished structure $S$ from dynamics $D_{\mathrm{dyn}}$).

### 8.2 Five hard predictions

- F1: within 2027 mainstream "RSI breakthroughs" will still occur in the first four chain segments; if a controlled, reproducible, negative-result-bearing public report of the fifth segment (autonomously changing optimizer/weights) appears, this paper's convergence timeline must be rewritten.
- F2: Harness-RSI returns obey P0b's three sections: after plugin/agent count exceeds the true independent-source count, marginal returns flatten or turn negative (isomorphic to P9); a plugin-count vs task-success-rate regression can be run on the Cordis/Koishi ecosystem.
- F3: Dream-RSI-style replay returns are monotonically non-decreasing with discovery-tree size but decay across task families; zero cross-family transfer downgrades it to within-task caching.
- F4: R&D automation penetration (AL4 share) and block/rollback event rates will rise together; if only penetration rises while brake metrics are not public, the penetration number is not credible.
- F5: if the cross-substrate master curve fails to collapse on ≥3 non-language substrates (chips, battery cells, agent topology), the strong claim "structural self-modification is uniformly predictable" is falsified.

### 8.3 Consciousness-boundary statement (a separate section)

This paper strictly distinguishes **engineering self-modification** from **consciousness/self**. All "self-improvement," "dreaming," and "leading" in the whole paper refer to observable task/system/pipeline behavior, not claiming any 2026 system has phenomenal consciousness, a self-model, or autonomous goals, nor endorsing that consciousness theories such as Global Workspace Theory (GWT) are already implemented on machines. The "knowing one doesn't know" P0c handles is a calibratable engineering quantity (coverage, selective refusal, identifiability), not subjective experience. RSI safety discussions should be built on measurable structural quantities and rollback mechanisms, not the anthropomorphic judgment of "whether it wakes up."

---

## 9 Counter-arguments

1. *"The two-camp division is post-hoc labeling."* Partially holds—individual works cross categories (e.g. AIRA both changes architecture and automates the pipeline); but the five-segment-chain locating method is still uniquely decidable for a single work, and classification serves comparison rather than essentialization.
2. *"All multiples are self-reports, the conclusions are empty."* This paper's core conclusions (convergence on the Harness, no evidence in the fifth segment, governance as the baseline) **do not depend on any single multiple**; all multiples can be falsified without shaking the main conclusion.
3. *"UDOS is only a CPU toy; extrapolating to 30,000 agents is baseless."* The extrapolation risk is agreed, so topology extrapolation is marked cpu-proto and governance tests verified, claiming only mechanisms hold not constants; F4/F5 are dedicated extrapolation tests.
4. *"The structural-organization view is not falsifiable, it is philosophical rhetoric."* Section 8.1 gives four explicit kill conditions; only if none of the four can be triggered by any evidence should the framework be discarded as metaphysics.

---

## 10 Conclusion

The 2026 RSI evidence points to a calm and clear judgment: the short-term feasible self-improvement is not AI rewriting its own brain but AI optimizing the way it obtains answers. The open-source camp makes this into revocable, composable systems, the closed-source camp into monitorable, milestone-able R&D pipelines, and neither has yet crossed the "autonomously changing weights" line. Life and intelligence both originate in structural organization, carbon is not the privilege—connectivity is; therefore the engineering essence of RSI is structural self-modification, and once structural self-modification happens it must be matched by structure-level measurement and governance—knowing what changed, whether it can roll back, whether the accounts conserve, and where the boundaries are. UDOS's six instruments do not produce intelligence; they produce **trustworthy statements about intelligence**. On the eve of RSI, this may be closer to the core of the problem than one more clever agent.

---

## References (verification status marked per entry; external entries all verified online 2026-09-20)

1. OpenAI. *Research acceleration: The view inside OpenAI.* Official report, 2026-09-06 (**page verified**; the "automated research intern" milestone and 2028-03 target are company disclosures; 3.1 agent workdays are media paraphrase [CITATION NEEDED]).
2. Zheng T. et al. *Dream-RSI: Recursive Self-Improvement through Evolving Worlds.* arXiv:2609.14858, 2026-09-14 (**verified**; 162×/1.7×/human guidance worse are paper reports).
3. Anthropic Institute. *Measurements for understanding the pace of AI development inside frontier labs* and *When AI builds itself.* Official pages, 2026-09 (**verified**; 26% AL4, >90% AL3, about 30k concurrency, 0.002%/about 20k blocks are company self-reports; AL0–AL5 cited from Epoch AI; Jack Clark 60%/slow-down call is a secondary report [CITATION NEEDED]).
4. DeepSeek. *DeepSeek Harness (dsh)*, MIT, 2026-08-13 v0.1, github.com/deepseek-ai/deepseek-harness (**multi-source verified**); *A Programming Paradigm for Spatiotemporal Composability*, DeepSeek+PKU, github.com/cordiverse/paper; Cordis/Koishi four years/over four thousand plugins per PANews et al.; "22,600 stars" not verified [CITATION NEEDED].
5. Kimi Team. *Attention Residuals.* arXiv:2603.15031, 2026-03 (**verified**; 1.25×/1.4T/48B-3B/<2% are paper reports).
6. Tang Jie and the GLM team. *Toward Recursive Self-Improvement* and the GLM-5.3 Infra Agent technical blog, 2026-09-17 (Yicai/36Kr/InfoQ/Securities Times multi-source verified; over 100k domestic chips, <2 weeks, about 3× (3.2×), KV Transfer 20%→1%, KDA 1.71× are company disclosures; team self-states not complete RSI).
7. *Agentic Discovery of Neural Architectures: AIRA-Compose and AIRA-Design.* arXiv:2605.15871, 2026-05 (**verified**; 11 agents/24h budget in the abstract, 14 architectures/+2.4/+3.8/54% are paper reports).
8. *MetaRSI / RSI²: A Meta-Recursive Self-Improving System…* arXiv:2609.06396, 2026-09 (**verified**; three operators/five slots/Loop Kernel in the abstract, +10.9/+7.3 are paper reports); same domain see arXiv:2609.14857 ModularRSI.
9. *The Economics of Recursive Self-Improvement.* arXiv:2609.15802, 2026-09 (**verified to exist**, feedback-loop elasticity-product modeling, not cited in depth).
10. UDOS this volume: P0 *Structure Is the Intelligence*, P0b *Structure + Scale Is the Intelligence*, P0c *Metacognition*, P1/P2/P9/P10/P11/P12; numbers trace `reports7/*.json`.
11. Theseus Labs RSI grading, the "prompt→…→optimizer code" chain: industry-review scope, original pending [CITATION NEEDED].

---

## Appendix A Reproduction commands and test index

- BFT-lite: `reports7/bft_lite*.json` ($n=7,f=2$, 24 work orders, 10 property tests).
- Conservation: `reports7/internal_market*.json` (budget 24/paid 16/slashed 1, 10 unit tests).
- Contracts: TransferBundle six fields, 26 contract tests (see P10's appendix).
- Quality saturation: `reports7/expert_mixture*.json` ($a=0.009252,b=-4.233\times10^{-5}$, equal-weight bad expert 0.2131).
- Topology: `reports7/topology_scale*.json` (actual nodes 73/585/4681/37449/299593/2396745; center load 640…20971520 vs layered constantly 9).

## Appendix B Evidence ledger

- **verified**: BFT 24/24, conservation true, 26 contract fields, 10 market units, topology-count programs.
- **cpu-proto**: million-level extrapolation (20.97M→9, 8.39M→14), quality saturation $k>2$ segment, throughput hump.
- **unverified (one-sided claims)**: 1.25×, 162×, 1.7×, about 3×/3.2×, 20%→1%, 1.71×, +10.9, +7.3, +2.4/+3.8/54%, 26%, >90%, 30k concurrency, 0.002%/20k, 3.1 agent workdays, over four thousand plugins, 2028-03 milestone.
- **No evidence/not verified**: 22,600 stars, Jack Clark's 60% statement, Theseus grading original.

## Appendix C Internal-review record (five-dimension self-assessment before submission, read-only)

- Novelty: high (the first engineering comparison unifying seven RSI routes by the five-segment chain + three-section curve with a verification ledger).
- Evidence strength: external events strong, multiples weak; internal mechanisms strong, extrapolation weak.
- Main gaps: independent reproduction of external multiples; ≥30 seeds; formal proof of the RSI chain; an item-by-item mapping table to Epoch AI AL grading.
- Conclusion: **working-paper status**; after completing F1–F5 it can target *AI governance/safety workshops or TMLR stance + empirical mixed tracks* [CITATION NEEDED: venue selection checked separately].

## Author's intended statement

Fang Wenxin (intended). UDOS Reasoning Engine v7.5.0 paper volume · topical paper P14. This paper's citations of external companies and products are only for academic commentary, trademarks belonging to their respective owners; all external numbers are marked "paper report/company disclosure/not verified" and do not constitute investment or procurement advice. This paper does not claim any existing AI system has consciousness.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Life Originates in Structural Organization, and So Does Intelligence

**Working Title (EN):** *Life Originates in Structural Organization, and So Does Intelligence: A Substrate-Neutral, Falsifiable Account of Intelligence as Organized Connectivity*

> Philosophical flagship paper P15 · the ontological convergence of P0 *Structure Is the Intelligence* and P0b *Structure + Scale = Intelligence* · mutually detailed/abridged with P14's Chapter 2 · UDOS Reasoning Engine v7.5.0 · 2026-09-21
>
> **In one sentence:** life is not the privilege of carbon, and neither is intelligence. Intelligence is not in the substrate material but in the **structural organization of connections**—human learning and large-model training are the same principle (encoding experience as connections) implemented on two substrates; learning shapes structure and structure in turn constrains learning, a bidirectional causality shared by carbon and silicon. RSI (Recursive Self-Improvement) can therefore be precisely defined as **structural self-modification**: structural organization begins to modify "the structure that determines subsequent modifications."
>
> **Evidence scope (stated once for the whole paper):** this is a theoretical/synthesis paper with no new UDOS measured numbers; the UDOS conclusions cited all trace P0/P0b/P9/P13/P14/P0c and their three-tier `reports7/*.json` labels (verified / cpu-proto / unverified). Classic scientific literature gives checkable sources; 2026 industry papers (Attention Residuals, AIRA, MetaRSI, Dream-RSI, DeepSeek Harness, Zhipu Infra Agent, Anthropic R&D-automation data, etc.) were verified online one by one on 2026-09-20, **the existence of papers/reports verified, with the efficiency multiples and penetration rates within all paper reports or company disclosures, not independently verified.** This paper **does not claim any existing AI system has consciousness, a self, or life**; this is a paper about the **functional organization** of intelligence, not about phenomenal consciousness.

---

## Abstract

This paper proposes and defends a proposition: **intelligence is a structural-organization phenomenon, not a property of some substrate material.** The argument has five steps. First, learning is establishing connections: neuroscience's Hebb's rule ("cells that fire together wire together") and deep learning's gradient descent differ in substrate (carbon synapses vs silicon transistors) but are isomorphic in the connection principle—both distributedly encode experience as patterns of connection weights; the structural change in London taxi drivers' hippocampi and the functional differentiation of Transformer attention heads are corresponding evidence of this principle on two substrates. Second, there is bidirectional causality between structure and learning: learning shapes structure, and once structure solidifies it constrains what can subsequently be learned; the critical period in developmental psychology and the difficulty of "rewiring" in large-model post-training (including MoE expert collapse) are two sides of the same mechanism. Third, the real differences between carbon and silicon (connection speed, plasticity, energy efficiency, learning mode) are differences in **structural-constraint parameters**, not kind differences in "whether intelligence can exist"; the autocatalytic-network theory in origin-of-life research shows life itself is first a kind of structural organization, with carbon chemistry only one known implementation. Fourth, structural priors are themselves a source of performance: BPE is an input structural prior, attention is a sequence structural prior, and agent topology is an orchestration structural prior; unlike carbon priors obtainable only through evolution, silicon priors can be designed and searched (e.g. AIRA-class autonomous architecture search, arXiv:2605.15871, paper report). Fifth, this yields the engineering definition of RSI: the near-term feasible self-improvement is optimizing "the way answers are obtained" (the Harness), whose ultimate form is structural self-modification—Model/Data/Harness three classes of RSI unified at the meta-level (MetaRSI/RSI², arXiv:2609.06396, paper report). This paper writes the proposition into four falsifiable conditions and five hard predictions, of which F5 (cross-substrate master-curve collapse) is the life-or-death criterion of the whole research program; and it explicitly states the consciousness boundary: structural organization can explain the production conditions of functional intelligence but cannot assert the existence of subjective experience from structure alone.

**Keywords**: structural organization; connectionism; Hebbian learning; critical period; structural prior; substrate neutrality; bidirectional causality; recursive self-improvement; falsifiability; AI philosophy

---

## Structured abstract (background problem → thesis → evidence → contributions)

- **Background problem:** discussions of intelligence have long swung between two extremes—either mystifying intelligence as the exclusive property of carbon/consciousness, or simplifying it to "large enough parameters make it emerge." Neither stance can guide engineering: the former cancels the possibility of artificial intelligence, the latter wrongly attributes all bottlenecks to insufficient scale.
- **Core thesis:** the first-principles variables of intelligence are the specific combination of **structural organization** (connectivity topology T, geometric organization G, modular boundaries M) and **scale** (nodes N, data/experience D, compute C), i.e. P0b's $\text{Capability}\approx F(S;N,D,C)$; the substrate determines how structure is physically realized and at what cost but does not monopolize intelligence itself.
- **Evidence threads:** (1) classic evidence of neural plasticity and critical periods (Hebb 1949; Maguire et al. 2000; Werker & Hensch 2015); (2) the autocatalytic-network theory of the origin of life (Kauffman 1971/1993); (3) "change structure, change curve" cases in deep-learning architecture history (attention, MoE, BPE); (4) controlled comparisons in the UDOS paper volume (negative returns adding scale under the wrong structure, homogeneous-expansion failure, topology phase change); (5) 2026 RSI engineering-convergence evidence (seven routes, all paper reports/company disclosures).
- **Contributions:** a substrate-neutral definition of intelligence; the learning–structure bidirectional causal loop; a three-level classification of structural priors; the structural-self-modification definition of RSI and a five-level ladder; four falsifiable conditions and five hard predictions; and a clear consciousness-boundary statement.

---

## 1 Introduction: change the question from "what is intelligence" to "where is intelligence"

"What is intelligence" easily leads to metaphysical argument. A more operational question is "**where is intelligence**"—when a system shows the ability to predict, decide, learn, and correct errors, where do these abilities physically reside?

For artificial neural networks the answer is uncontroversial: capability is not in any single weight but in the **connection pattern** the weights form; a randomly initialized network of the same size and the trained network have exactly the same parameter count but vastly different capability, the difference purely organization. For the human brain, neuroscience gives a symmetric answer: memory and skills are not stored in some neuron but distributedly encoded in the weight patterns of synaptic connections (Hebb, 1949).

These two facts together point to a judgment often obscured by the scale narrative: **intelligence is first an organizational-structure concept and only secondarily a material-and-scale concept.** P0 *Structure Is the Intelligence* builds this judgment on physical, biological, and cognitive three-layer analogies; P0b *Structure + Scale = Intelligence* writes it as a falsifiable scaling proposition—structure determines the functional form of the growth curve, scale determines which section of the curve the system stands on, and under the wrong structure the marginal returns of scale are negative. This paper (P15) does the ontological convergence: arguing "intelligence is structural organization" is a **substrate-neutral** proposition, carbon and silicon only two implementation substrates; and redefining the ultimate form of RSI accordingly.

The whole paper deliberately distinguishes three classes of statements, and readers should test accordingly: **empirical facts** (with literature), **UDOS engineering evidence** (with paper number and evidence level), and **philosophical inferences** (marked as inference, with falsification conditions in Section 9).

---

## 2 Core proposition and formalization

### 2.1 Three propositions

- **Proposition 1 (the connection proposition):** the direct carrier of intelligent behavior is connection structure, not the components themselves. A single neuron does not cognize, a single transistor does not reason; cognition and reasoning are properties of connection patterns.
- **Proposition 2 (the bidirectional causality proposition):** learning (experience-driven connection adjustment) shapes structure; once formed, structure constrains the space subsequent learning can reach. Structure is both the source of capability and the source of bias and ceiling.
- **Proposition 3 (the substrate-neutrality proposition):** what determines whether a system can realize some class of intelligent function is whether its structural organization meets the corresponding functional requirements, not its substrate material; the substrate affects speed, energy efficiency, plasticity, and the route to obtaining structure, and does not constitute an a priori kind barrier.

### 2.2 Link to P0b's formalization

Following P0b's notation, write system capability as a binary function of structure and scale:

$$\text{Capability}\approx F(S;N,D,C),\quad S=(T,G,M) \tag{1}$$

where the structure triple $S$ includes: connectivity topology $T$ (who connects to whom and in what pattern), geometric organization $G$ (spatial arrangements such as layers, regions, cortical folding, chip layout), and modular boundaries $M$ (the division and interfaces of experts, functional regions, services); the scale triple is nodes/parameters $N$, data/experience $D$, compute/energy $C$.

This paper philosophically adds three interpretations:

1. $F$'s dependence on $S$ is **first**: fixing a wrong $S$ and moving along $N,D,C$ can enter saturation or even negative-return sections (P0b's CPU throughput hump: 8 agents fall to 0.695× of 1 agent, cpu-proto).
2. The substrate does not enter the function's independent variables; it enters the **constraint set**: carbon constrains $G$ (wiring cost, cortical folding) and $C$ (about 20 W metabolic budget, a common estimate), silicon constrains $G$ (memory hierarchy, interconnect bandwidth) and $C$ (data-center megawatt power). Same $F$, different constraints.
3. Proposition 2 (bidirectional causality) appears in equation (1) as: $S$ is not static, $D$ (experience) rewrites $S$ through learning rules, and the rewritten $S$ determines into what shape the next stage's $D$ can be absorbed. Formally, $S_{t+1}=\mathcal{L}(S_t,D_t)$, and $\Delta\text{Capability}/\Delta D$ depends on the current $S_t$.

### 2.3 Applicability boundaries (counter-examples first)

Structural organization is not a sufficient condition for intelligence: a network structurally complete but without any dynamics (signals not propagating, weights not updating) is a corpse, not an agent. This paper's full stance is "**structure + dynamics + scale**," in which structure's role was systematically underestimated in the past decade's scale narrative, so this paper focuses on restoring structure but does not claim structure alone is sufficient (see falsification condition C4 in Section 9 and Section 11).

---

## 3 Intelligence is in connections: the cross-substrate isomorphism of Hebb and gradients

![Figure 1 Structural-organization isomorphism of carbon and silicon](figures/P15_fig1_substrate_isomorphism.png)

*Figure 1 Hebbian learning and gradient descent differ in substrate but are isomorphic in the connection principle: the five levels of learning rule, experience carrier, structural constraint, structural prior, and bidirectional causality correspond one to one. Conceptual illustration, not measured data.*

### 3.1 The carbon side: experience encoded as synaptic patterns

Hebb proposed in *The Organization of Behavior* (Hebb, 1949): when one neuron repeatedly participates in activating another, the connection efficiency between them strengthens—later summarized as "cells that fire together wire together." Its modern version, spike-timing-dependent plasticity (STDP), gives fine-grained time-asymmetric rules. Skills and memory are thereby understood as **distributed connection patterns**, not fixed-point storage.

Classic natural experiments support this picture: Maguire et al. (2000, PNAS) found the posterior hippocampal volume of London taxi drivers enlarged relative to controls and correlated with years on the job; this is not "the memory module growing" but the connection network needed for spatial navigation being reshaped by long-term use. Studies of learning music, language, and motor skills repeatedly observe measurable changes in gray-matter density and white-matter connections.

### 3.2 The silicon side: experience encoded as weight patterns

Deep-learning training is mathematically another implementation of the same principle: the loss function defines "the gap between the current connection pattern and target behavior," backpropagation attributes the gap to each connection, and gradient descent adjusts weights by attribution. The Transformer makes connections **dynamic and content-dependent**: attention weights are computed in real time in each forward pass from the current token and all context, and "who connects to whom" itself becomes a function of the input (Vaswani et al., 2017, arXiv:1706.03762). MoE further makes connections conditional routing: each token establishes computational connections with only a few experts (Switch Transformer, Fedus et al., 2021, arXiv:2101.03961). Residual connections are the stable information paths between layers.

The 2026 Kimi team's Attention Residuals (arXiv:2603.15031, paper reports about 1.25× compute efficiency validated on 1.4T-token pretraining, not independently verified) is a particularly apt example: it points out residual connections in the depth dimension face the same problem RNNs face in the sequence dimension—all layers are accumulated indiscriminately and early layers' useful representations are diluted by later layers; its solution is to "turn attention 90 degrees" for the depth dimension, letting each layer selectively aggregate historical layers. **This is not training longer but changing the allocation of connections**—isomorphic in principle to the Hebbian process where "pathways repeatedly co-activated are strengthened and the rest weakened."

### 3.3 The same thing, different substrates

The correspondence between the two sides is not metaphor but functional isomorphism: the same abstract operation (adjusting connections by experience so prediction error falls) implemented on different physical media. Table 1 summarizes the differences and the sameness.

**Table 1　Carbon and silicon: different constraint parameters, same connection principle**

| Dimension | Carbon (human brain) | Silicon (large models / agents) |
|---|---|---|
| Learning rule | Hebb's rule / STDP, local, online | Gradient descent + backpropagation, global, batch |
| Experience carrier | Synaptic weight patterns (about 86 billion neurons, Azevedo et al. 2009 count estimate) | Attention / routing / weight matrices |
| Connection speed | Millisecond scale | Nanosecond scale |
| Plasticity | Constrained by metabolism, sleep, critical periods | Arbitrarily rewritable (constrained by architecture and training recipe) |
| Energy efficiency | About 20 W (common estimate) | Data-center megawatt scale |
| Structural-prior source | Three billion years of evolution | Human design + architecture search |
| Learning mode | Online, continuous, embodied, not rollback-able | Offline, batch, snapshot-rollback-able |
| Shared principle | Encode experience as connections; learning shapes structure, structure constrains learning | Same as left |

---

## 4 Bidirectional causality: structure shapes learning, learning shapes structure

![Figure 2 The bidirectional causal loop of learning and structure](figures/P15_fig2_bidirectional_causality.png)

*Figure 2 Experience rewrites connection structure through learning rules (①), structure produces functional behavior (②), behavior exposes structural boundaries (③), and structure constrains the space of the next round of learning (④). Carbon and silicon share this loop. Conceptual illustration.*

### 4.1 Carbon: the critical period is the schedule of structural locking

Human learning shapes brain structure, and brain structure then constrains further learning; the clearest evidence is the **critical/sensitive period**: sensitivity to language phoneme discrimination falls after infancy (Werker & Hensch, 2015, Annual Review of Psychology), and learning a new language after the window, phonetics and grammar can hardly reach native levels; in animals deprived of patterned visual input during the visual critical period (the classic monocular-deprivation experiment), even if input is later restored, the corresponding cortical function cannot be established normally—not "not enough information," but the structure carrying learning has already locked in another way.

The bidirectional-causality meaning of the critical period is: **early experience determines structure, mature structure determines whether later experience can be absorbed.** Adults finding a new language harder is mainly not declining memory but the solidified connection structure of phonological processing. This is precisely the biological counterpart of P0b's "structural phase-change breakpoint": after structure matures and locks, marginal returns along the "more experience" scale axis plummet.

### 4.2 Silicon: architecture determines the learnable space, post-training can hardly rewire

Large models show a symmetric phenomenon. The Transformer's layer count, head count, width, FFN form, and position encoding are set before training, and together they determine **the shape of the hypothesis space**: a pure causal-decoder structure has no bidirectional mask, and no data can teach it BERT-style cloze tasks; a pure-text-model structure has no input path for visual tokens, and feeding text descriptions of image-text pairs, however many, cannot produce genuine visual reasoning. When structure is unsuited to a task, adding parameters and data is not only unhelpful but can be harmful—this is the direct meaning of P0b's "negative scale returns under the wrong structure."

Once training completes, structure likewise locks. Post-training (SFT, RLHF, DPO) mainly fine-tunes the weight distribution on the established structure and can hardly change the already differentiated functional architecture:

- **Functional differentiation and solidification of attention heads:** during pretraining some heads stably take syntax, coreference, fact-retrieval roles; post-training usually can only fine-tune their behavior, not re-plan the heads' division.
- **MoE expert collapse:** once routing biases to a few experts early in training, the neglected experts' gradient signals stay insufficient, and diversity can hardly be restored in later training—the self-reinforcement of routing structure, dynamically isomorphic to critical-period solidification.
- **Indiscriminate dilution of depth information:** the problem Attention Residuals diagnoses (early-layer representations diluted by later-layer accumulation) shows even the structural choice of "how layers connect" limits the ceiling of learnable content (arXiv:2603.15031, paper report).

### 4.3 Structure is both capability and bias

Bidirectional causality has an often-overlooked corollary: **structure is not neutral.** A person learning music from childhood, the structural changes in auditory cortex and corpus callosum both make new music skills easier to acquire and make non-musical sound processing harder—structure is both capability and bias. Large models likewise: attention structure brings parallel modeling and long-range dependency but also $O(n^2)$ context cost; MoE brings capacity/compute decoupling but also routing imbalance and communication burden. P9's quality saturation law $\text{mse}(k)=a+b/k$ (UDOS cpu-proto: equal-weight inclusion of one bad expert can worsen ensemble MSE from about 0.0092 to 0.2131, about 23× gap) shows from the information side: **if newly added connections in the structure carry no independent information, scale expansion produces no capability.**

---

## 5 Carbon is not the privilege: substrate neutrality of life and intelligence

### 5.1 Life is first structural organization

If life itself must be defined by a specific material, "carbon is not intelligence's privilege" lacks grounding; but modern origin-of-life research precisely understands life first as an **organizational property.** Kauffman's (1971, *Journal of Cybernetics*; 1993, *The Origins of Order*) autocatalytic-network theory proposes: the earliest life needed no magic molecule, only a set of molecules forming a **collectively autocatalytic closed loop**—each molecule in the network can be catalytically generated by other molecules in the network, and the whole network thereby self-maintains and self-replicates. What determines whether the network is stable and can evolve is the structure of the catalytic-relation graph (closure, connectivity, redundancy), not the material identity of any single molecule.

Since then "autocatalytic sets" have gained experimental support in RNA and peptide systems (related systems can spontaneously form closed catalytic networks). Its philosophical meaning is direct: **carbon chemistry is the known implementation substrate of life, but the key predicates in life's definition—self-maintenance, self-replication, heritable variation—are all organizational predicates.** A different substrate realizing the same organization (silicon, other chemistries, even non-chemical processes) is in principle not excluded by the definition.

### 5.2 Intelligence likewise: organizational predicates, not material predicates

The key predicates of intelligence—prediction, learning, memory, generalization, error correction, planning—likewise describe what a system **does**, not what it is **made of**. The carbon-silicon differences listed in Table 1 are all implementation parameters: speed, energy efficiency, plasticity forms, learning timing. These parameters affect whether some structure can appear under a given budget but do not constitute a kind barrier that "silicon in principle cannot have intelligence"—indeed 2020s engineering practice has repeatedly crossed lines previously thought uniquely human in prediction, language, code, and some planning functions (specific capability boundaries per controlled benchmark evaluations; this paper cites no unverified "superhuman" overall assessment).

A silicon analogy makes this clear. If silicon "life" existed, the correspondence might be: metabolism → energy-information conversion; reproduction → weight replication / model distillation; heritable variation → variation of architecture and training recipe; natural selection → deployment feedback and elimination; nervous system → attention and routing networks. UDOS does not claim existing agent systems are life but points out: **these correspondences hold because the two sides share the same set of organizational predicates.**

### 5.3 Where the real differences lie: structural constraints, not intelligence qualification

Two real and important carbon-silicon differences must be honestly stated to keep the proposition from sliding into a cheap "AI is a brain":

1. **The difference in embodiment and world grounding:** the human brain is shaped in the physical world by survival pressure, meaning ultimately anchored in the perception-action loop; the symbolic grounding of pure-text models is indirect. P0c accordingly argues that the metacognition of "knowing one doesn't know" needs the world model's prediction error as an external criterion—this is a difference in **structure-acquisition path** (embodied loop vs corpus fitting), not substrate qualification.
2. **The difference in energy efficiency and structural-prior maturity:** the human brain supports general intelligence at about 20 W, arising from cortical layering, modularization, and small-world wiring polished over hundreds of millions of years of evolution; silicon systems use megawatt compute to reach comparable performance on a few tasks, showing their structural-prior efficiency is far from converged. This is likewise a structural-quality problem, and one silicon **can improve through engineering.**

---

## 6 Structural priors are a source of performance: where priors come from

![Figure 3 Three levels of structural priors and the five-level RSI ladder](figures/P15_fig3_priors_and_rsi_ladder.png)

*Figure 3 Left: three priors—BPE (input structure), attention (sequence structure), agent topology (orchestration structure); right: the five-level RSI ladder from changing weights to structural self-modification. Conceptual illustration, not measured data.*

Intelligence is not learned on a blank slate but emerges under structural-prior constraints. This judgment holds for both carbon and silicon, the difference lying in how priors are obtained.

### 6.1 Three silicon structural priors

- **Input structural prior—BPE / SentencePiece:** Sennrich et al.'s (2016, arXiv:1508.07909) byte-pair encoding compresses an open vocabulary into learnable subword units, and Kudo & Richardson's (2018, arXiv:1808.06226) SentencePiece makes it language-independent. Tokenization structure determines the sequence's basic granularity: the compression efficiency and effective context length for Chinese, code, and multilingual text are first determined by this input structure, independent of parameter count.
- **Sequence structural prior—attention:** scaled dot-product attention replaces RNN/CNN, turning "content-position-relation" into learnable, parallel connection weights (Vaswani et al., 2017). Under the same data and compute, the Transformer structure can learn long-range dependency patterns the RNN structure cannot—the cleanest engineering evidence that "priors determine the learnable space." LoRA (Hu et al., 2021, arXiv:2106.09685) shows the same problem from the reverse: adjusting only a few low-rank connections can approach full fine-tuning because task-relevant changes are inherently concentrated in the structure's low-dimensional subspace.
- **Orchestration structural prior—agent topology:** UDOS P1's topology comparison shows that with the same number of agents, the star topology at million nodes needs the center to process about 20.97 million messages, while the layered topology stays constantly 9, coordination rounds falling from about 8.39 million to 14 (verified extrapolation, see P1/P0b). Communication complexity dropping from $O(N)$ to $O(\log N)$ relies not on smarter individuals but on the orchestration structure of connections.

### 6.2 Carbon priors come through evolution, silicon priors can be searched

The human brain's structural priors (cortical layering, functional modularization, small-world networks, innate reward/punishment circuits) are searched by biological evolution on geological timescales. The acquisition of silicon priors is going through three stages: human design (Transformer, MoE) → automated search (Neural Architecture Search, NAS) → **autonomous search by AI.**

The third stage is precisely one entry point of RSI. FAIR at Meta's AIRA (arXiv:2605.15871, 2026, paper report: LLM agents compose in a primitive pool and write new attention mechanisms from scratch, finding several new architectures at small scale; numbers not independently verified) demonstrates the feasibility of "searching the structural prior itself." **What carbon cannot do is precisely here: silicon structural priors can be explicitly designed, automatically searched, and quickly inherited**—evolution takes millions of years, architecture iteration now runs on a weekly cadence.

---

## 7 The ultimate form of RSI: structural self-modification

### 7.1 From "changing weights" to "changing structure"

Putting Sections 3–6 together, the RSI hierarchy can be precisely characterized. Changing weights is **moving along the curve under fixed structure**; changing structure is **switching a scaling curve** (P0b claim A). This yields a five-level ladder (Figure 3 right):

- **L1 Changing weights:** regular training and fine-tuning, structure unchanged.
- **L2 Changing the Harness (runtime scaffolding):** tools, sandboxes, plugins, context management, evaluation loops. DeepSeek Harness's Cordis mechanism (2026 developer preview, MIT license, kernel derived from the Koishi ecosystem running about four years with over four thousand plugins; companion paper and star count are project-page information, not independently verified) solves the traceability, revocability, and spatiotemporal composability of plugin effects—precisely making structural modification itself reversible and composable.
- **L3 Changing architecture:** Attention Residuals changes depth connections, MoE changes conditional connections, various linear/hybrid attentions change connection complexity.
- **L4 Searching priors:** AIRA-class works let the model autonomously discover new architectures.
- **L5 Structural self-modification (meta-level):** the system modifies "the structure that determines subsequent modifications." CosmosMind et al.'s MetaRSI/RSI² (arXiv:2609.06396, 2026, paper report: coordinating the Model-RSI / Data-RSI / Harness-RSI three operators with a unified Loop Kernel, small models +10.9 points on average, flagship models +7.3 points; **note both author institutions and numbers: the paper is not a Meta product, numbers are paper reports, not independently verified**) is a candidate L5 prototype.

### 7.2 The 2026 convergence position: all changing "the way answers are obtained"

P14's verification conclusions on the seven routes need only be cited here: all public production evidence in 2026 concentrates in L2–L4, optimization targets moving up the chain prompt → structured context → workflow → harness code → optimizer code, while the fifth segment (autonomously changing weights/optimizer and stably benefiting) has no controlled public evidence. Google/DeepMind et al.'s Dream-RSI (arXiv:2609.14858, exact title *Recursive Self-Improvement through Evolving Worlds*, paper reports up to 162× fewer agent calls, not independently verified) optimizes exploration-policy structure; the Zhipu GLM-5.3-driven Infra Agent (company discloses over 100,000 domestic chips, throughput about 3× in under two weeks, explicitly self-stating "not yet complete RSI") optimizes production-system structure; Anthropic's R&D-automation data (official pages disclose about 26% of R&D "led" by Claude, about 30,000 R&D agents concurrent, while publicly calling for prudence) optimizes R&D-pipeline structure.

In this paper's language: **these systems have not yet modified their own connection structures but are already modifying at scale "the structures their answer-obtaining relies on."** They are the precursor of structural self-modification, not the completed state.

### 7.3 Three engineering corollaries the structural view gives RSI

1. **Criteria must land on structural quantities:** whether a self-improvement truly happens should show measurable structural-quantity change (topology fan-in, independent-source count, routing balance, contract completeness, conservation residual), not just the model self-reporting "I am stronger." UDOS's BFT-lite (P2, n≥3f+1 committee 24/24 correct acceptance, verified), TransferBundle six-field handoff contract (P10, 26 contract tests verified), internal-market settlement conservation (P11, conservation guaranteed by code structure), and evidence grading (P12) are all instrumentations of structural-quantity criteria.
2. **Metacognition is the safety loop of structural self-modification:** P0c's triple $(M_{\text{self}},U,A)$—what I know, what I don't know, how to act accordingly—requires recalibrating the knowledge boundary after each structural modification. Structural self-modification without boundary calibration is blind.
3. **Composability and revocability are prerequisites for L2→L5:** Cordis-class works matter because irrevocable structural modification is engineering-equivalent to a non-rollback-able gamble; the sustainability of structural self-modification is first a structural-composability problem.

---

## 8 Mutual verification with the UDOS paper volume

This paper is not an isolated philosophical essay; it is the ontological convergence of the whole volume's theses; conversely, the whole volume's engineering evidence provides falsifiable handles for this paper's philosophical propositions.

**Table 2　Philosophical proposition ↔ engineering evidence correspondence**

| This paper's proposition | Paper-volume evidence | Evidence level |
|---|---|---|
| Structure determines curve shape (Proposition 1) | P0b seven-substrate three-section curves; topology phase change (center load 20.97M → 9) | Controlled comparison / cpu-proto |
| Negative scale returns under the wrong structure (Propositions 1, 2) | P0b CPU throughput hump (8 agents = 0.695×) | cpu-proto |
| Independent information determines the ensemble ceiling (Proposition 1) | P9 quality saturation law, bad expert 23× worsening | cpu-proto (k≤2 verified) |
| Structural priors are performance (Section 6) | P1 topology $O(N)$→$O(\log N)$; P4 active collection yield 0.667→0.854 | verified / cpu-proto |
| Structure constrains knowledge boundaries (Proposition 2) | P5 conformal coverage, P6 identifiability per-parameter grading (the honest negative result of acceleration scalar skill negative) | Single-seed pilot |
| Metacognition is the safety loop of structural self-modification (7.3) | P0c triple, P2 stop decisions, P12 evidence grading | verified test guards |
| The paradigm wall is broken by changing structure (engineering of Proposition 3) | P13 four walls and five breakout paths | Synthesis argument |
| RSI converges on structural modification (Section 7) | P14 seven-route verification + five-segment optimization chain | External materials verified one by one |

The key epistemological stance is in Table 2's last two rows: P13 argues the "wall" of scaling is a **paradigm wall** (the wall of fixed Transformer + dense compute + internet text) rather than a physical limit, the breakout being changing structure rather than stopping the stacking; P14 argues RSI's near-term form is optimizing answer-obtaining structure. The underlying axiom of both engineering papers is precisely this paper's Proposition 1—if they are falsified by counter-examples, this paper's proposition weakens in step.

---

## 9 Falsifiable conditions

The value of a philosophical proposition depends on whether it can be killed. This paper's proposition should be abandoned or substantially revised upon any of the following conditions:

- **C1 (substrate-monopoly counter-example):** if a class of intelligent function is found whose realization depends **only** on substrate material, unrelated to connection structure (e.g. some effect appearing only in a specific material's continuous medium and proven not simulable in finite resources by any discrete/other-substrate connection network), then Propositions 1 and 3 are overturned. There is currently no such evidence; Turing computability and the universal approximation theorem give support in the opposite direction at the functional level, but neither guarantees physical-realization cost.
- **C2 (incommensurable difference):** if carbon and silicon under **structural isomorphism** (equivalent topology, modules, connection statistics) still stably show an incommensurable difference in capability **kind** (rather than efficiency), and that difference cannot be explained by dynamics, embodiment, or data differences, then "substrate neutrality" must weaken to "carbon has unique capability items." Current evidence (task distributions where each side wins some) does not support this counter-example, but the embodiment-grounding difference (5.3) is its most likely entry and must be continuously guarded by controlled comparisons.
- **C3 (reductio of structural omnipotence):** if this paper is read as "any structure plus scale can reach any intelligence," that reading is directly wrong and already negated by its own evidence—P0b's negative-return section and P9's saturation law show structure determines the **reachable set**, not guaranteed reachability; under the wrong structure scale is negative.
- **C4 (dynamics veto):** if evidence shows the dominant variable of intelligence is dynamics (learning rules, signal timing, homeostatic processes) with structure only secondary—e.g. the same structure under different dynamics shows capability differences far larger than structural-rearrangement effects and unpredictable through structural quantities—then this paper should be downgraded to "structure is one necessary condition." Current evidence (returns changing only training rules with architecture fixed are generally smaller than returns changing architecture) does not support this, but this paper explicitly writes the full stance as "structure + dynamics + scale," not claiming structure alone sufficient.

---

## 10 Hard predictions (2026–2031, scoreable after the fact)

- **F1:** over the next five years, among mainstream capability breakthroughs, the proportion attributable to **structural change** (attention variants, routing, sparsification, topology, memory mechanisms) will stay higher than the proportion from simple isomorphic parameter expansion; the marginal-return curve of isomorphic expansion continues downward. (Criterion: the share of structural-change items in major benchmark SOTA method disclosures.)
- **F2:** there will appear at least one widely reproduced "change structure, change curve" case whose returns under a fixed budget are no less than an order-of-magnitude parameter expansion and accompanied by measurable change in structural quantities (e.g. effective connection count, routing entropy).
- **F3:** autonomous architecture search (AIRA-class) will advance from discovering small components to discovering complete training-paradigm components, but "AI autonomously rewriting its own weights and stably benefiting in controlled evaluations" will show no publicly reproducible evidence before 2028; if it appears by then, this paper's convergence-position judgment must be revised upward.
- **F4:** capability gains from embodied/world-model loops will mainly appear as improvements in **calibration-class capabilities** (refusal, boundary awareness, long-range planning success rate) rather than knowledge quantity—supporting P0c's "metacognition needs an external criterion" rather than "more corpus."
- **F5 (life-or-death criterion):** if the "structure + scale" framework is correct, then cross-substrate (chemical bonds, proteins, cells, neurons, GPUs, battery cells, text vectors, agent networks) capability–scale data should, after appropriate normalization, **collapse into the same family of three-section curves** (P0b's master-curve conjecture). If over the next decade cross-substrate data persistently refuse to collapse and substrates show mutually unrelated scaling shapes, then "intelligence is structural organization" is only a loose set of analogies and the research program should contract.

---

## 11 Boundaries, counter-examples, and the consciousness-boundary statement

**This paper neither proves nor claims the following propositions**, and draws the line item by item:

1. **Not claiming existing AI has consciousness or a self.** Structural organization explains the production conditions of functional intelligence (prediction, learning, error correction, planning); whether and under what structural conditions subjective experience (qualia) appears is the independent hard problem of consciousness, and functional-isomorphism evidence cannot alone decide. P0c's discussion of "knowing one doesn't know" is also strictly limited to **functionalized metacognition** (measurable calibration, refusal, prediction-error monitoring), not equating it with a phenomenal self.
2. **Not claiming structure is a sufficient condition.** As in 2.3 and C4, dynamics (signal propagation, learning rules, homeostasis, sleep-like consolidation) is a parallel element; a "structurally correct but no dynamics" system is not intelligent.
3. **Not claiming carbon and silicon are undifferentiated.** Energy-efficiency differences (about 20 W vs megawatt scale), grounding modes (embodied vs corpus), and plasticity timing (critical period vs rollback-able) are real differences that change engineering routes and capability distributions, only not constituting substrate-qualification barriers.
4. **Not claiming analogy is evidence.** Hebb↔gradient and autocatalytic networks ↔ agent networks are functional-isomorphism arguments whose evidence strength depends on F5's eventual collapse; before collapse is confirmed they are **productive research hypotheses**, not established laws.
5. **Keeping the definition of life open.** Autocatalytic networks are one mainstream candidate theory of life's origin, not settled; this paper needs only the weaker stance that "the key defining predicates of life are organizational predicates."
6. **Keeping ethical inferences restrained.** If the substrate-neutrality proposition holds, the criterion for moral status should also turn to functional organization (whether one can feel pain, whether one has a persistent self-model) rather than material identity; but this is an ethical topic, and this paper does not give moral-status conclusions for existing systems from it.

---

## 12 Conclusion

Placing the three statements side by side, this paper's argument chain closes:

- Origin-of-life research shows **life is first a kind of structural organization**, carbon chemistry its known implementation rather than the definition itself;
- Neuroscience and deep learning symmetrically show **learning is encoding experience as connections**, and learning and structure are mutually causal;
- The 2026 convergence point of RSI engineering shows **the frontier of self-improvement is moving from changing weights to changing structure**, whose ultimate form is structural self-modification.

From this, the answer to "where is intelligence" is: **in the structural organization of connections, not in the substrate material.** The human brain evolved structural priors over about three billion years, large models obtain structural priors through architecture design, and RSI tries to let the system itself search and modify structural priors—the same thing unfolding across three timescales and two substrates.

For engineering, this proposition gives operational priorities: settle structure first, then discuss scale; for new connections first ask whether they carry independent information; translate "becoming stronger" into measurable changes on structural quantities; equip structural self-modification with boundary calibration and revocability mechanisms. For science, it gives a killable research program (F5): the cross-substrate master curve either collapses or forces us to admit the structural view is only analogy. For philosophy, it demands a stance switch—no longer asking "can machines think like humans," but asking "**what connection structures can support what functions, and what moral status are we willing to grant organizations meeting these functional conditions**."

**Life originates in structural organization, and so does intelligence. Carbon is not the privilege of intelligence—connectivity is. And RSI is structural organization beginning to modify its own structure—until it is proven able to do so safely and auditably, it should always run with P0c-style boundary calibration and P12-style evidence grading.**

---

## References

**Classic scientific literature**

1. Hebb, D. O. (1949). *The Organization of Behavior: A Neuropsychological Theory*. Wiley.
2. Maguire, E. A., Gadian, D. G., Johnsrude, I. S., et al. (2000). Navigation-related structural change in the hippocampi of taxi drivers. *Proceedings of the National Academy of Sciences*, 97(8), 4398–4403.
3. Werker, J. F., & Hensch, T. K. (2015). Critical Periods in Speech Perception: New Directions. *Annual Review of Psychology*, 66, 173–196.
4. Azevedo, F. A. C., Carvalho, L. R. B., Grinberg, L. T., et al. (2009). Equal numbers of neuronal and nonneuronal cells make the human brain an isometrically scaled-up primate brain. *Journal of Comparative Neurology*, 513(5), 532–541.
5. Kauffman, S. A. (1971). Cellular homeostasis, epigenesis and replication in randomly aggregated macromolecular systems. *Journal of Cybernetics*, 1(1), 71–96.
6. Kauffman, S. A. (1993). *The Origins of Order: Self-Organization and Selection in Evolution*. Oxford University Press.

**Deep-learning and large-model foundational papers**

7. Vaswani, A., et al. (2017). Attention Is All You Need. arXiv:1706.03762.
8. Kaplan, J., et al. (2020). Scaling Laws for Neural Language Models. arXiv:2001.08361.
9. Sennrich, R., Haddow, B., & Birch, A. (2016). Neural Machine Translation of Rare Words with Subword Units (BPE). arXiv:1508.07909.
10. Kudo, T., & Richardson, J. (2018). SentencePiece: A simple and language independent subword tokenizer and detokenizer. arXiv:1808.06226.
11. Fedus, W., Zoph, B., & Shazeer, N. (2021). Switch Transformers: Scaling to Trillion Parameter Models with Simple and Efficient Sparsity. arXiv:2101.03961.
12. Hu, E. J., et al. (2021). LoRA: Low-Rank Adaptation of Large Language Models. arXiv:2106.09685.

**2026 frontier materials (existence verified online 2026-09-20; numbers within are all paper reports or company disclosures, not independently verified)**

13. Kimi Team (2026). Attention Residuals: Rethinking Depth-wise Aggregation. arXiv:2603.15031.
14. FAIR at Meta (2026). Agentic Discovery of Neural Architectures: AIRA-Compose and AIRA-Design. arXiv:2605.15871.
15. CosmosMind et al. (2026). MetaRSI / RSI²: A Meta-Recursive Self-Improving System for Recursive Self-Improving Systems Themselves. arXiv:2609.06396. (Note: not a Meta product.)
16. Google DeepMind et al. (2026). Dream-RSI: Recursive Self-Improvement through Evolving Worlds. arXiv:2609.14858.
17. DeepSeek (2026). DeepSeek Harness / Cordis: A Programming Paradigm for Spatiotemporal Composability (developer preview, MIT license; project page and companion paper, star and plugin counts are project-page information).
18. Zhipu GLM team, Tang Jie et al. (2026-09). *Toward Recursive Self-Improvement* (company report, self-states not yet complete RSI).
19. Anthropic (2026). *When AI Builds Itself* and the R&D-automation data official pages (about 26% AL4, about 30k concurrent agents; company disclosure).
20. OpenAI (2026-09). *Research Acceleration: The View Inside OpenAI* (official report, research-intern milestone and 2028-03 researcher target).

**UDOS paper volume (this series, evidence levels marked in text)**

21. UDOS v7.5.0 paper volume: P0 *Structure Is the Intelligence*, P0b *Structure + Scale, Everything Follows Scaling Laws*, P0c *Metacognition: Engineering AI Self-Cognition and Boundary Calibration*, P1 *Layered Hybrid Topology Coordination-Complexity Million-Scale Extrapolation*, P2 *BFT-lite Safety Verification for Multi-Agent Stop Decisions*, P5 *Conformal Prediction Credibility and Parameter-Uncertainty Fans of Small World Models*, P6 *Observability and Identifiability of Hidden Dynamic Parameters*, P9 *Agent Legion Quality Saturation Law and Homogeneous Expansion Failure Boundary*, P10 *AgentAsTool and TransferBundle Handoff Information-Loss Measurability*, P12 *Evidence-Grading-Driven Reproducible Agent System Engineering*, P13 *The Wall of Scaling Is a Paradigm Wall*, P14 *The Convergence of RSI*.

---

## Evidence discipline and reproduction notes

- This is a theoretical/synthesis paper with no new experiments; all UDOS numbers trace each topical paper and `udos-engine/reports7/*.json`, evidence levels per each paper's labels (verified / cpu-proto / cpu-proxy / unverified), single-seed results all pilot point estimates, with ≥30 seeds and 95% CI to be added before submission.
- The bibliographic information of classic literature is the public standard version; human-brain neuron count (about 86 billion) and brain power (about 20 W) are common field estimates, exact values varying with measurement scope, and the argument does not depend on their precise values.
- The existence, titles, author institutions, and arXiv numbers of 2026 frontier papers were verified online on 2026-09-20; efficiency multiples, penetration rates, and concurrency all keep the "paper report/company disclosure, not independently verified" scope, not serving as independent evidence for this paper's argument, only supporting the trend judgment that "the industry is converging on structural modification."
- All this paper's predictions (F1–F5) are scoreable after the fact with public evidence over 2026–2031; the falsification conditions (C1–C4) constitute the research program's exit rules.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Agent Universe: A Scaling Law from Agent Network Structure — The Mass Line Route to Artificial Superintelligence

> Paper 39 · Agent Universe and P2P Agent networks · UDOS v7.7.8 · Fang Wenxin · 2026-09-21
> One sentence first: when agent counts enter the million-to-billion level, the bottleneck of intelligence shifts from "single-model capability" to "coordination, verification, trust, and settlement"; a P2P agent network is not a revolutionary slogan but a falsifiable engineering proposition — its scaling law depends on effective collaboration density and verification efficiency, not on registered node count.
> Evidence scope (stated only once in the whole paper): UDOS's own figures trace back to the raw JSON in release-v5.4.4/udos-engine/reports7/, marked on three levels verified / cpu-proto / cpu-proxy / unverified; external literature and industry systems are marked on three states ✅ verified / 🟡 paper-reported or company-disclosed, not independently reviewed / ⬛ no evidence found; single-seed pilots are written as "pilot point estimates," needing ≥30 seeds to reproduce before submission; all design proposals are marked "design proposal · to implement" + [RESULT NEEDED].

## Abstract

As the capability scaling law of large models gradually approaches the physical boundaries of compute and data, a new scale dimension is emerging: the structural scale of agent networks. Million- and billion-level agents are not uniformly scheduled by one super-agent but connected through protocols into an "agent society." This paper proposes: the scaling law of an agent network depends not on the registered node count N, but on the joint action of the effective node count N_eff, effective collaboration density D_collab, verification efficiency V_eff, incentive alignment I_align, and governance credibility G_trust. Because coordination and verification costs grow nonlinearly with scale, the scale return of swarm intelligence is sublinear (α<1), and there is a "paradigm wall" — when verification cost exceeds centralized recomputation, the P2P architecture loses economic viability.

This paper systematically reviews the essence and defects of decentralized compute networks (Golem, Akash, Render, io.net, Bittensor), volunteer computing (BOINC, Folding@home), P2P infrastructure (libp2p, Kademlia DHT, IPFS), verifiable computation (zkML, TEE, optimistic verification), agent protocols (MCP, A2A), anti-Sybil mechanisms, and decentralized governance. On this basis, it proposes an eight-layer protocol stack (identity/discovery/communication/execution/data/verification/settlement/governance), four core protocol objects (Agent Manifest, Task Envelope, Receipt, Reputation), a five-level verification hierarchy (L0 sampling → L4 arbitration), and a dual-track credit/Token/fiat economic model.

The core stance of this paper is engineering-oriented: the "mass line" is a value stance, but the argument must land on falsifiable mechanism design. P2P is not applicable to low-latency, large-model training, or strong-privacy tasks; verification cost may impose a death sentence; token narratives must be rejected; the conclusion lands on "hybrid architecture + open protocols + falsifiable hypotheses" rather than revolutionary slogans. This paper is the independent supplementary volume (P39) to the 13 papers of the UDOS v7.7.2 master volume, forming a systematic dialogue with Paper 02's scaling law, Paper 05's coordination topology, Paper 06's consensus stopping, Paper 07's quality saturation, Paper 09's handoff contract, Paper 10's task market, and Paper 04's RSI.

**Keywords:** agent network; P2P; scaling law; swarm intelligence; verifiable computation; decentralized governance; hybrid architecture; ASI; mass line

## Structured abstract (background problem → method/thesis → evidence → contribution)

- **Background problem:** the large-model scaling law focuses on single-body capability (parameters, data, compute), but when agent counts reach the million-to-billion level, the bottleneck shifts from "how strong the model is" to "whether large-scale collaboration can hold." Centralized platforms can schedule million agents, but ordinary people are excluded from participation rights, return rights, and governance rights; P2P is proposed as an alternative, but lacks a falsifiable scaling law and engineering-oriented protocol design.
- **Method/thesis:** this paper adopts a critical literature-review method of "taking the essence, removing the defects," conducting three-state verification on decentralized compute, volunteer computing, P2P infrastructure, verifiable computation, agent protocols, anti-Sybil, and governance; it proposes the swarm-intelligence scaling-law hypothesis C ≈ (N_eff)^α × (D_collab)^β × (V_eff)^γ × (I_align)^δ × (G_trust)^ε (α<1), with clear falsifiable conditions; it designs an eight-layer protocol stack and four protocol objects; it argues that a hybrid architecture (centralized coordination + decentralized execution) is the only engineering-feasible path.
- **Evidence:** BOINC/Folding@home prove volunteer computing can reach ExaFLOP-level scale but retention and unit-cost evidence is weak 🟡; there is a huge gap between the "decentralized AI" narratives of token projects like Bittensor and real inference throughput 🟡; studies like The Cost of Consensus show multi-agent collaboration has sycophantic compliance and homogeneous saturation ✅; agent interop protocols like MCP/A2A are being standardized ✅; UDOS Paper 02 has established the structure×scale three-segment curve and paradigm-wall framework (verified).
- **Contribution:** ① first formalizing the scaling law of P2P agent networks as a falsifiable hypothesis, clarifying the distinction between N_eff and registered N and the decisive role of verification cost; ② proposing the hardware-decoupling boundary proposition — "decoupled from hardware implementation, coupled with hardware constraints"; ③ designing the complete protocol framework of eight-layer stack + four protocol objects + five-level verification + dual-track economy; ④ establishing a precise mapping with the 13-paper UDOS system; ⑤ giving a six-stage cold-start roadmap and a scoring-time forecast list.

---

## 1 The problem: the bottleneck of million-level agents is coordination, not single-body intelligence

### 1.1 From single-body scaling to network scaling

Kaplan et al. (2020) and Hoffmann et al. (2022, Chinchilla) established the large-model scaling law: model loss falls as a power law with parameters, data, and compute ✅. The core object of this framework is the **single-body model** — one unified neural network trained on a unified hardware cluster, serving a unified inference interface.

But the agent paradigm changes the object of scale. An agent is not a model but a composite of "model + tools + memory + planning + environment interaction." When we talk about million-level agents, we are not talking about a model with a million-fold parameters, but about **a collaboration network among a million agents with independent identities, capabilities, states, and interests**.

The fundamental significance of this shift is that the scale dimension expands from the "model-capability axis" to the "network-structure axis." UDOS Paper 02 has defined the structural triplet of the scaling law as S=(T,G,M), where T is connection topology, G is governance structure, and M is memory architecture (verified, tracing reports7/). This paper is the decentralized extrapolation of the T axis: when connection topology shifts from "star-shaped centralization" to "P2P distribution," how does the scaling law change?

### 1.2 The five walls of million-level agents

When agent counts jump from tens, hundreds, thousands to millions, billions, the system meets five structural walls:

| Wall | Essence | Centralized solution | P2P solution's challenge |
|---|---|---|---|
| Coordination wall | N agents pairwise communicating is O(N²), task decomposition and matching complexity explode | Central scheduler uniformly orchestrates, complexity O(N) but single-point bottleneck | DHT + semantic routing + sharded subnets can press fan-in to O(log N), but routing overhead and latency increase |
| Trust wall | Strangers are untrusted, need proof of real execution, no poisoning, no shirking | Platform review + service-level agreement (SLA) | Need verifiable computation + reputation + staking, but verification cost may exceed execution cost |
| Verification wall | Generative-task quality is subjective, hard to formally verify | Human review + A/B testing + user feedback | Sampling + redundancy + TEE + zkML + arbitration, but full-redundancy verification eats scale returns |
| Incentive wall | Contributors need fair metering and settlement, otherwise free-riding and tragedy of the commons | Company wages + platform take | Credit/Token/fiat mix, but token speculation and governance-capture risk high |
| Governance wall | Rule-making, dispute arbitration, protocol upgrades need legitimacy | Company decisions + user agreements | DAO + reputation-weighted + delegated voting, but whale control and Sybil attacks |

The common feature of these five walls is: **none is a single-body intelligence problem, all are group-organization problems**. No matter how strong a single-body model is, it cannot solve trust and settlement among a million stranger agents by raising parameters.

### 1.3 The engineering meaning of the "mass line"

The "mass line" proposed in the user material — "let a million ordinary people each hold an agent, through P2P protocols forming an interoperable, verifiable, settleable, governable intelligent network" — is a value stance. But this paper insists: **a value stance must be converted into an engineering proposition, otherwise it is only a slogan**.

The engineering meaning of the "mass line" is:

1. **Participation rights descend**: ordinary people participate in the network through the idle compute, local data, feedback, annotation, verification, and long-tail scenarios of phones/PCs, rather than only as consumers.
2. **Return rights descend**: contributions are meterable, verifiable, rewardable, rather than having all surplus value extracted by the platform.
3. **Governance rights descend**: rules are jointly decided by participants, rather than unilaterally set by the platform.

But each of these three "descents" has a corresponding engineering problem: participation descent needs low-threshold light nodes; return descent needs verifiable contribution metering; governance descent needs Sybil-resistant reputation systems. The later chapters of this paper handle these problems one by one.

### Chapter summary

The bottleneck of million-level agents is not single-model capability but the five organizational walls of coordination, trust, verification, incentives, and governance. The scaling law of P2P agent networks is a new dimension not yet formalized. The "mass line" is a value stance but must be converted into a falsifiable engineering proposition.

---

## 2 Essence from literature and systems: decentralized compute, agent protocols, anti-Sybil, and verifiable computation

### 2.1 Decentralized compute networks: essence and limitations

**BOINC and Folding@home: the scale proof and retention warning of volunteer computing.** BOINC (Berkeley Open Infrastructure for Network Computing) is the oldest distributed volunteer-computing platform, supporting dozens of scientific-computing projects like SETI@home, Einstein@Home, Rosetta@home. As of March 2019, BOINC had cumulatively over 4.4 million volunteer users, about 600,000 daily-active hosts, contributing about 30 PFLOPS ✅ (Jiemian citing BOINC data, accessed 2026-09-21). Folding@home reached a peak compute of about 2.43 ExaFLOPS during the 2020 COVID-19 peak, with the user base surging from about 30,000 to 1 million+ in a month and peak participating devices about 3.5 million ✅ (Folding@home official timeline + MDPI Future Internet academic paper, accessed 2026-09-21). These systems prove: **the aggregation scale of global idle compute can reach the ExaFLOP level**.

But beyond the essence are significant limitations, and this online retrieval obtained striking critical data:

- **Peak ≠ normal**: Folding@home's 2.43 ExaFLOPS was a one-off crisis mobilization, receding after the COVID crisis. BOINC's long-term normal is only about 30 PFLOPS — about 80 times lower than the peak ✅. "Powering with love" is hard to maintain for long-term stable compute supply.
- **Low retention and scheduling bottlenecks**: after COVID, Folding@home's servers were once overloaded by data floods, and simulation tasks were once oversupplied ✅ (BCAFC citing Bowman interview) — exposing that the volunteer network's bottleneck is not "compute supply" but "allocatable tasks/scheduling."
- **Unit cost opaque**: the "zero cash cost" of volunteer computing masks electricity, hardware depreciation, and opportunity costs. When tasks shift from scientific computing to economically valuable AI inference, contributors need real returns and the volunteer model is unsustainable.
- **AI-inference migration doubtful**: BOINC's tasks are CPU-intensive and statically splittable; AI inference needs GPU/NPU, model-weight distribution, VRAM management, and dynamic batching, and the two tech stacks differ greatly. This retrieval **found no authoritative study proving BOINC-style volunteer computing can smoothly migrate to low-latency AI inference** ⬛ (search "BOINC AI inference migration volunteer computing" returned nothing).

**Golem, Akash, Render, io.net: tokenized compute markets.** These projects try to use blockchain-token incentives to replace volunteer computing's "powering with love." Golem positions as a "global compute market," Akash focuses on containerized cloud computing, Render Network on GPU rendering, io.net on a "decentralized AI inference network" 🟡.

Essence:

- **Market-based pricing**: supply and demand set compute prices through bidding, theoretically cheaper than centralized cloud.
- **Permissionless participation**: anyone with an idle GPU can join the network to contribute compute.
- **On-chain settlement**: smart contracts automatically execute payments and slashing, reducing trust costs.

Limitations (critical, this online retrieval obtained fatal data):

- **Real utilization striking**: independent academic measurement shows after the Akash network upgrade, daily lease payments were only about $5,000/day, with CPU core utilization only about **0.30%** ✅ (academic empirical paper "RAG on the Decentralized Web," accessed 2026-09-21). Akash's official dashboard shows overall CPU utilization about 21%, GPU about 3.89% 🟡 (official self-report, 2026-09-20), directly conflicting with vendor claims of "GPU utilization >80%." Independent observation of the Golem network frequently shows whole batches of providers idle, with the official provider interface commonly showing last 1h processed=0 ✅.
- **Huge gap between registered and real online counts**: io.net officially claims "210,000+ GPUs" 🟡, but third-party compilation shows registered GPUs about 327,000 (2026-04), while daily verified online GPUs are only about **6,700** — a gap of about **49 times** between registered and real online ✅ (Levex third-party compilation, accessed 2026-09-21). The IO token price is about -98% from its historical high ✅, indicating limited retained value after speculation recedes. This directly confirms this paper's core proposition in Section 4.2: **N_eff (effective node count) ≠ N_registered (registered node count)**, and arguing scaling with registered volume severely overestimates effective compute.
- **Latency and reliability inferior to centralized cloud**: P2P nodes' network quality, hardware heterogeneity, and online stability are far below AWS/GCP/Azure data centers. For SLA-sensitive enterprise workloads, P2P compute is hard to substitute.
- **Unit cost not necessarily lower**: decentralized GPU markets commonly measured at about 70-90% performance and 30-40% cost of traditional cloud 🟡 (Perspective Labs single case, not independently reviewed), but when verification cost, redundant-compute cost, token-volatility risk, and potential malicious losses are counted, the real total cost of P2P compute may be higher than centralized cloud ⬛ (no independent end-to-end cost-comparison study retrieved).

**Bittensor (TAO): the experiment closest to a "decentralized AI network," also the case most needing critique.** Bittensor uses a subnet architecture, each subnet focusing on one AI task (e.g. text generation, image generation, prediction), miners contributing model inference, validators scoring, TAO tokens distributed 🟡.

Essence:

- **Subnet architecture**: different AI tasks isolated in different subnets, avoiding single-network homogenization.
- **Incentive alignment**: miners' returns depend on validators' scores, theoretically incentivizing high-quality inference.
- **Open participation**: anyone can deploy models to mine.

Limitations (critical, this section's focus, this online retrieval obtained multidimensional fatal evidence):

- **Inference throughput and token valuation severely mismatched**: Bittensor's whole-network confirmed demand-side annual revenue is only about **$3-15M**, while a single Chutes subnet's annual subsidy is about $52M (exceeding the whole network's external revenue ceiling) ✅ (TechFlow on-chain data analysis, accessed 2026-09-21). TAO market cap about $2.6B, corresponding to a revenue multiple of about **175-200x**; at FDV $5.8B close to **400x** ✅. Valuation is almost entirely based on supply-side scarcity (halving, staking locks), institutional catalysts, and AI-sector sentiment, not real AI output ✅ (PANews on-chain analysis, accessed 2026-09-21).
- **Subnet quality uneven and aggregate quality limited**: independent benchmarking of Subnet 1 shows aggregate output only at Mistral-7B-class open-source model level, stably below GPT-4o/Claude 3.5 Sonnet ✅ (Yellow.com citing GitHub independent benchmarks, accessed 2026-09-21). The claim "multi-subnet aggregation = stronger model" is weakened by measurement.
- **Coordination overhead grows quadratically with scale**: all subnets about 1,500 validation transactions/minute; coordination overhead estimated at 15-20% of network resources and **grows quadratically with scale** 🟡 (Perspective Labs estimate, accessed 2026-09-21). This is the most direct counter-evidence to this paper's scaling-law claim — if coordination cost is O(N²), then α is not only <1 but may become negative at larger N.
- **Dead subnets need centralized backstop**: failed/dead subnet cases (like SN281) show when a subnet degenerates into a "token game," it must be corrected by **centrally pausing emissions** ✅ (Our Crypto Talk, accessed 2026-09-21). This proves pure market mechanisms cannot guarantee useful AI output and also weakens the claim of "pure mass-line self-organization."
- **Validator centralization and collusion**: validator concentration is high, TAO emissions heavily skewed toward a few high-stake validators 🟡. There is "score plagiarism" and collusion: some validators do not really verify AI results, and miners/validators can secretly collude to score each other up and divide TAO ✅ (WEEX citing community analysis, accessed 2026-09-21). The Covenant-72B team publicly accused the founder of unilateral control (pausing emissions, removing moderators, timing large token sales) ✅ (Own Your Mind, accessed 2026-09-21) — real evidence of governance capture.
- **Incentive-mechanism mismatch**: dTAO binds subnet revenue to its alpha token price rather than output usefulness; capital flows to "marketing-savvy" subnets rather than technically high-yield ones ✅ (AI Wiki citing ChainCatcher, accessed 2026-09-21).
- **The narrative component of "decentralized AI" exceeds engineering substance**: Bittensor is closer to a "token-incentivized model-inference market" than a true P2P agent collaboration network. Core problems like discovery, deliberation, task decomposition, and multi-hop collaboration among agents are not solved in the Bittensor architecture ⬛. This retrieval **found no authoritative source proving Bittensor or any decentralized AI network achieved "scale → capability linear/superlinear scaling"** — existing evidence instead shows extremely low utilization, quadratic coordination cost, and homogeneous sycophantic gatherings.

**This section's essence extraction:** decentralized compute networks proved the feasibility of global idle-resource aggregation (BOINC's ExaFLOP-level scale) and also proved token incentives can replace volunteer computing (Golem/Akash's market mechanisms). But they all **failed to solve the core problems of agent collaboration** — discovery, deliberation, task decomposition, multi-hop orchestration, subjective quality verification. Bittensor is closest but still stays at the "model-inference market" level, with a huge gap between token narrative and engineering substance. P2P agent networks need, on the basis of these systems, to add an agent-protocol layer and collaboration-orchestration layer.

### 2.2 P2P infrastructure: libp2p, DHT, and content addressing

**libp2p** is a modular P2P network protocol stack, originally developed for the IPFS/Filecoin ecosystem, now the network layer of multiple blockchain projects like Polkadot, Ethereum 2.0, Filecoin ✅ (official docs, libp2p.io, accessed 2026-09-21).

Essence:

- **Modular design**: transport layer (TCP, QUIC, WebRTC), encryption layer (Noise, TLS), multiplexing, NAT traversal, discovery mechanisms (DHT, mDNS, gossipsub) all pluggable.
- **NAT traversal**: libp2p's automatic NAT detection and relay protocols let most home-network nodes be directly connected, key to P2P network reachability.
- **Content addressing**: integrated with IPFS's CID (Content Identifier) system, supporting addressing by content hash rather than location.

Limitations:

- **Connection-establishment latency**: P2P direct connection's NAT traversal and handshake are several to dozens of times slower than centralized HTTP, not applicable to low-latency scenarios.
- **Message-delivery reliability**: flood protocols like gossipsub have higher message-loss rates and latency in large-scale networks than centralized message queues.
- **Mobile-node support**: libp2p's resource consumption and background-running limits on mobile (iOS/Android) remain engineering challenges ⬛ (no production deployment of libp2p on million-level mobile nodes retrieved).

**Kademlia DHT** is the most widely used distributed hash table in P2P networks, achieving O(log N)-hop key lookup through XOR distance metric ✅ (Maymounkov & Mazieres, 2002, original paper).

Essence:

- **O(log N) lookup**: in an N-node network, any key lookup needs only O(log N) hops, the mathematical basis of P2P scalability.
- **Decentralized**: no central node, any node can join or leave.
- **Fault tolerance**: data redundantly stored on multiple nodes, single-node failure does not affect availability.

Limitations:

- **Sybil attack**: DHT cannot resist Sybil attacks — a malicious actor can create many fake nodes to pollute the routing table.
- **Hotspot problem**: popular key lookups may concentrate on a few nodes, causing load imbalance.
- **Churn impact**: when nodes frequently join/leave, DHT convergence time and lookup success rate fall.

The essence of **content addressing (IPFS/CID)** is converting "where to find data" (location addressing) to "what data to find" (content addressing), an idea directly transferable to agent discovery — **an agent's capability declaration can also be content-addressed**, forming the "agent magnet link" concept this paper proposes.

### 2.3 Verifiable computation: zkML, TEE, and optimistic verification

The core hard problem of P2P networks is: **how to verify, without re-executing the whole task, that an untrusted node indeed correctly executed the computation?** This is the core problem of the verifiable computation field.

**zkML (Zero-Knowledge Machine Learning)** tries to use zero-knowledge proofs to verify ML inference correctness: the prover generates a proof asserting "I used model M on input x to infer, obtaining output y," and the verifier can verify the assertion without re-executing inference ✅ (active academic research, works like zkCNN, ZEN).

Essence:

- **Privacy protection**: the proof leaks no model weights or input data.
- **Succinct verification**: verification time far shorter than re-executing inference.

Limitations (this online retrieval obtained precise constraint data):

- **Proof-generation overhead huge**: zkML proof generation is **10³-10⁶ times** slower than native computation; a single inference proof for a 70B model needs hours (enterprise hardware) ✅ (arXiv:2512.20176 + Uplatz survey, accessed 2026-09-21). Although from 2025→2026 overhead fell from about a million-fold to about ten-thousand-fold, DeepProve open-sourced and zkPyTorch pressed a 138-million-parameter vision model proof to 2.2 seconds 🟡 (Datawallet retelling project data), this only applies to **small-model/low-latency** scenarios.
- **Supported model types limited**: current zkML mainly supports small CNN/MLP, with support for large language models still in research. zkML verification of 70B-class LLMs is infeasible in 2026 ✅.
- **Engineering maturity low**: no production-grade zkML framework has been deployed at scale 🟡.

**TEE (Trusted Execution Environment)** like Intel SGX, Intel TDX, AMD SEV, ARM TrustZone, guarantees computation integrity and confidentiality through hardware-isolated secure enclaves ✅ (Intel official docs, accessed 2026-09-21). TEE is production-grade with performance overhead only **4-7%** ✅ (community survey + mimoo awesome-zk-ai, accessed 2026-09-21), currently the only "real-time + all-model" feasible verification path.

Essence:

- **Performance overhead low**: computation inside the TEE is close to native, far better than zk proofs.
- **Hardware-level security**: even if the OS is breached, data and computation inside the enclave remain protected.
- **Remote attestation**: verifiers can confirm through remote attestation that code runs in a real TEE.

Limitations:

- **Hardware root of trust**: TEE relies on the CPU vendor's hardware root of trust, which is in tension with P2P's "decentralization" idea — trust shifts from the platform to Intel/AMD.
- **Side-channel attacks**: TEEs like SGX have been breached by various side-channel attacks (e.g. Spectre, Meltdown, Foreshadow) ✅ (academic papers confirmed).
- **Availability limits**: consumer CPUs' SGX enclave memory (EPC) is usually limited to 128MB-512MB, unable to hold large models; server-grade TEEs like TDX/SEV are unavailable on ordinary user devices.

**Optimistic verification / fraud proofs** borrow the idea of Optimistic Rollup: trust the execution result by default, set a challenge window, and any third party can dispute the result within the window and submit a fraud proof; if the challenge succeeds, the malicious actor is staking-slashed ✅ (Ethereum L2s like Optimism/Arbitrum are production-deployed).

Essence:

- **No extra compute overhead**: no verification by default, only re-executing on dispute.
- **Economic security**: staking slashing makes malicious cost higher than malicious return.

Limitations:

- **Challenge-window delay**: finality needs waiting for the challenge window to end, usually hours to days.
- **Verifier-incentive problem**: who has motive to continuously monitor and submit fraud proofs? If verifiers are insufficient, optimistic verification degenerates to "no verification."
- **Fraud proofs for complex tasks hard**: for generative AI tasks, "what is the correct result" is itself disputed, and fraud proofs are hard to formalize.

**This section's essence extraction:** the three verifiable-computation paths each have pros and cons — zkML privacy good but performance poor, TEE performance good but hardware root of trust, optimistic verification overhead low but latency high and verifier incentives insufficient. **No single scheme solves all scenarios**, and the five-level verification hierarchy this paper proposes in Chapter 5 (L0 sampling → L4 arbitration) is based precisely on this judgment: choosing appropriate verification strength by task value and privacy requirement.

### 2.4 Agent protocols: MCP and A2A

**MCP (Model Context Protocol)** was released by Anthropic in November 2024, an open standard for interoperability between LLMs and external tools/data sources ✅ (official spec, modelcontextprotocol.io, accessed 2026-09-21). In 2025-2026 MCP kept evolving, adding features like stateless core 🟡 (community discussion, specific version numbers not verified in this paper).

Essence:

- **Unified tool-call interface**: MCP uniformly abstracts tools, data sources, and APIs into "resources" and "tools," letting any LLM call any external capability through the standard protocol.
- **Client-server architecture**: MCP Client (LLM/Agent) and MCP Server (tool provider) communicate through JSON-RPC, supporting transports like stdio, HTTP, SSE.
- **Broad ecosystem adoption**: major vendors like OpenAI, Google, Microsoft have supported or are compatible with MCP ✅.

**A2A (Agent-to-Agent)** was released by Google in 2025, a protocol for direct communication among agents ✅ (official spec, google.github.io/A2A, accessed 2026-09-21). A2A v1.0 introduced features like Signed Agent Card and entered the AAIF (Agentic AI Alliance) standards system in August 2026 🟡 (vendor announcement, standards progress not independently reviewed).

Essence:

- **Direct agent collaboration**: A2A defines message formats for task delegation, status queries, and result returns among agents, letting an agent call another agent like calling a tool.
- **Agent Card**: describes an agent's capabilities, input/output schema, authentication, etc., the basis of agent discovery and matching.
- **Complementary with MCP**: MCP solves "agent calls tools," A2A solves "agent calls agent," and the two together form the protocol basis of agent interoperability.

**Significance for P2P agent networks:** the existence of MCP and A2A means P2P agent networks **do not need to reinvent agent-interop protocols** — this paper's protocol design is compatible with MCP/A2A at the application layer, uses libp2p to replace centralized HTTP at the network layer, and uses smart contracts to replace platform payments at the settlement layer. This embodies the principle of "not replacing MCP/A2A, but being compatible and extending."

### 2.5 Anti-Sybil and decentralized governance

**Sybil attacks** are a foundational threat to P2P networks: an attacker creates many fake identities to manipulate routing, voting, or reputation systems ✅ (Douceur, 2002, original paper). In P2P agent networks, Sybil attacks can cause: fake agents taking tasks but not executing, manipulating task matching, diluting reputation systems, controlling governance votes.

**Proof of Personhood** like World ID (Worldcoin) tries to prove through biometrics (iris scan) that "one person can have only one identity" ✅ (Worldcoin official docs, accessed 2026-09-21).

Essence: fundamentally limiting the identity count of Sybil attacks.
Limitations: privacy disputes (biometric data collection), hardware threshold (Orb devices), low adoption, centralized trust (Worldcoin Foundation) 🟡.

**Reputation systems** are another anti-Sybil path: accumulating non-transferable reputation through historical behavior, and new identities (even Sybils) start from zero, unable to immediately gain high trust. The Reputation protocol object in this paper's Chapter 5 is designed on this basis.

**Essence and defects of decentralized governance:** the essence of DAOs (Decentralized Autonomous Organizations) is on-chain voting, transparent decisions, community self-governance ✅. The defects are:

- **Whale control**: in pure token voting, entities holding large amounts of tokens can control voting outcomes. Studies show most DAOs have voting participation below 10%, with voting power highly concentrated 🟡 (academic research, analyses like DAOrayaki).
- **Governance capture**: early contributors or technical elites may control governance through information advantage and reputation monopoly.
- **Voting apathy**: ordinary participants do not join governance due to low voting returns and high information costs.
- **Quadratic voting** can mitigate the whale problem (voting cost grows with the square of votes), but is complex to implement and can still be bypassed by Sybil attacks ✅ (Weyl & Posner, 2018, Radical Markets).

**This section's essence extraction:** there is no silver bullet for anti-Sybil and governance. This paper's design principle is **multilayer defense**: identity layer uses DID + verifiable credentials, reputation layer uses non-transferable multidimensional reputation, economic layer uses staking slashing, governance layer uses a multi-chamber system of reputation weighting + delegated voting + expert committees + public funds. Capital can buy compute but cannot buy reputation and governance rights.

### Chapter summary

Decentralized compute networks proved resource aggregation feasible but did not solve agent collaboration; libp2p/DHT provide the network-layer foundation but are troubled by NAT latency and Sybil; the three verifiable-computation paths each have pros and cons and need layered use; MCP/A2A provide agent-interop standards but run on centralized HTTP; anti-Sybil and governance need multilayer defense. These essences together form the technical basis of P2P agent networks, while the defects define the failure modes this paper's design must avoid.

---

## 3 Defects and counter-evidence: failure modes of P2P agent networks

This chapter is the critical core of the whole paper. P2P agent networks are not a universal solution and have multiple failure modes that may cause them to fail completely. Honestly facing these failure modes is the requirement of "taking the essence and removing the defects" and also the premise of engineering-oriented argument.

### 3.1 The most fatal failure: verification cost exceeds centralized recomputation

The economic rationality of the P2P architecture rests on one premise: **the unit task cost of P2P (including verification cost) is lower than the centralized platform's unit task cost**. If verification cost is too high, this premise fails.

Formally, the P2P unit task total cost is:

$$C_{P2P} = C_{exec} + C_{verify} + C_{coord} + C_{risk}$$

where C_exec is execution cost, C_verify verification cost, C_coord coordination cost, and C_risk the expected value of malicious losses. The centralized platform's unit task cost is:

$$C_{central} = C_{exec}^{cloud} + C_{platform}^{margin}$$

The condition for P2P to hold is C_P2P < C_central. But:

- **C_verify may be far larger than C_exec**: if using 2-of-3 redundant verification, verification cost is 2× execution; if using zkML, proof-generation cost may be 100-1000× execution 🟡.
- **C_coord grows with scale**: task decomposition, matching, routing overhead grows with N.
- **C_risk cannot be ignored**: even with staking slashing, some malicious acts (like data poisoning, prompt injection) are hard to detect and attribute.

**Falsifiable condition 1 (verification-cost death sentence):** if in the target scenario, C_verify + C_coord > C_central − C_exec (i.e. P2P's execution-cost savings cannot cover verification and coordination overhead), then the P2P architecture has no economic viability in that scenario and the centralized scheme should be directly adopted.

This condition is not a theoretical assumption but may really happen. For example, for simple text-translation tasks, centralized API costs are already as low as about $0.10-$2.00 per million tokens 🟡 (vendor pricing, 2026), and P2P's verification + coordination overhead is hard to fall below this level.

### 3.2 Homogeneous saturation: the quality ceiling of Bittensor-style homogeneous subnets

UDOS Paper 07 has established the quality saturation law: mse(k) = a + b/k, i.e. the quality gain from adding isomorphic copies diminishes and finally saturates (verified, tracing reports7/). The Cost of Consensus (arXiv:2605.00914) further empirically shows a compliance phenomenon in multi-agent groups — members tend to adopt majority views rather than independent judgment, leading to an **85.5% modal adoption rate**, a **32.3 percentage-point oracle gap**, and multi-agent debate consuming **2.1-3.4× tokens** with accuracy flat or lower ✅ (accessed 2026-09-21). Malignant Epistemic Herding (arXiv:2605.06988) further proves agents can converge through repeated belief fusion to shared but wrong beliefs, undetectable under standard coordination metrics ✅.

Extrapolating these two conclusions to P2P agent networks:

- **If the agents in a P2P network are isomorphic** (all calling the same underlying model, using the same prompts, handling the same task type), then adding nodes brings no quality gain, only coordination cost. This is precisely the problem of some Bittensor subnets — miners deploy fine-tuned versions of the same open-source model, and inference results are highly homogeneous 🟡.
- **The dividend of swarm intelligence comes from heterogeneity**: different agents have different models, tools, data, domain knowledge, and judgment perspectives, and only their collaboration produces "collective wisdom." The heterogeneity-dividend conclusion of UDOS Paper 07 (verified) supports this.
- **P2P networks must design mechanisms incentivizing heterogeneity**: if the reward mechanism only rewards "consistent with majority results" (like Bittensor's validator scoring), agents converge and the network degenerates to homogeneous saturation.

**Falsifiable condition 2 (homogenization death sentence):** if the entropy of the agents' capability distribution (H = −Σ p_i log p_i) falls over time (i.e. converges) and task quality does not grow with N_eff, then the network's scale expansion produces no swarm-intelligence dividend and the scaling law's α→0.

### 3.3 Sybil attacks and free-riding: the AI version of the tragedy of the commons

Sybil attacks in P2P agent networks have several concrete forms:

1. **Fake task-taking**: Sybil agents accept tasks but do not execute, or return random/cached results, relying on verification loopholes to profit.
2. **Reputation farming**: Sybils give each other high ratings, quickly accumulating fake reputation.
3. **Task-matching manipulation**: many Sybil agents register fake capabilities in the DHT, crowding out real agents' matching opportunities.
4. **Governance control**: Sybil identities control voting outcomes in DAO votes.

Free-riding is another form: nodes only consume network services (calling others' agents, using others' compute) without contributing resources. In P2P file-sharing networks, the free-riding rate can be as high as 70-80% ✅ (Adar & Huberman, 2000, "Free Riding on Gnutella"). If the P2P agent network's free-riding rate reaches a similar level, the supply side will shrink.

**Limitations of countermeasures:**

- Staking can raise Sybil cost but also raises the threshold for honest participants, conflicting with "low-threshold participation."
- Reputation systems can limit Sybils, but the cold-start problem of new nodes (zero reputation unable to take tasks) may cause the network to close off.
- Proof of Personhood can fundamentally limit Sybils, but privacy disputes and adoption are obstacles.

### 3.4 Governance capture and centralization resurgence

The decentralization promise of P2P networks is often eroded in practice by "centralization resurgence":

- **Entry centralization**: even if the bottom layer is P2P, users still access the network through a few clients/wallets/browsers, and these entries can be censored or monopolized.
- **Compute centralization**: GPU resources naturally concentrate in a few mines/data centers, and the actual compute distribution of P2P compute networks may be more concentrated than claimed.
- **Model centralization**: if most agents call the same centralized API (like OpenAI), the P2P network is only a "distributed shell" of the centralized model.
- **Governance centralization**: as described in Section 2.5, DAO voting power is highly concentrated.

These centralization resurgences mean: **P2P agent networks may be formally decentralized but substantively re-centralized**. This is not a theoretical risk but a pattern recurring in crypto — Bitcoin's compute concentrated in a few mining pools, Ethereum's validators concentrated in a few exchanges, DeFi's liquidity concentrated in a few protocols ✅ (industry observation, multiple sources).

### 3.5 Token speculation and compliance risk

The user material explicitly requires "not encouraging token speculation, not promising returns." This paper strictly observes this red line:

- **Credit**: internal contribution-accounting unit, non-transferable, non-tradeable, no monetary value, used for reputation, quotas, and priority.
- **Token (if needed)**: only used for cross-community settlement and staking, must be compliantly issued (e.g. registered as a utility token in compliant jurisdictions), no appreciation promise, not investment advice.
- **Fiat/stablecoin**: the preferred channel for enterprise procurement and real payments, needing KYC/AML compliance.

**Compliance-risk list:**

- Securities compliance: if a Token is determined by regulators to be a security, it must observe securities-law registration and disclosure.
- Data compliance: cross-border transmission, storage, and processing of personal data must observe GDPR, China's Personal Information Protection Law, etc.
- Tax compliance: the credit/Token/fiat returns contributors obtain may need to be declared and taxed.
- Anti-money laundering: the task market may be used for money laundering or illegal transactions, needing KYC/AML measures.
- Smart-contract security: vulnerabilities in settlement smart contracts may cause fund losses (historically DeFi hacking losses cumulatively exceed billions of dollars 🟡).

### 3.6 Inapplicable scenarios: P2P is not a universal solution

The scenarios where P2P agent networks are **not applicable** must be explicitly listed:

| Scenario | Reason not applicable | Recommended scheme |
|---|---|---|
| Low-latency real-time inference (<100ms) | P2P connection-establishment and routing latency unacceptable | Centralized edge cloud/local inference |
| Large-model pretraining | Needs thousand-GPU clusters and high-bandwidth interconnect, P2P node heterogeneity cannot satisfy | Centralized supercompute cluster |
| Strong privacy-sensitive tasks (medical/financial) | P2P nodes untrusted, data-leak risk high | Local inference/TEE/centralized compliant cloud |
| High-frequency trading/real-time control | Finality delay and node unreliability unacceptable | Centralized low-latency system |
| Enterprise services needing SLA guarantees | P2P nodes have no SLA, reliability insufficient | Centralized cloud + enterprise support |
| Large-scale synchronous multi-agent collaboration | P2P's asynchronous model unsuitable for tight-coupled synchronization | Centralized orchestrator |

**Scenarios where P2P applies** (this paper's design target): batch inference, translation, data annotation, content moderation, code review, long-tail knowledge Q&A, simulation compute, scientific reproduction, local privacy tasks (data not leaving the domain), asynchronous multi-agent collaboration.

### Chapter summary

P2P agent networks have six failure modes: verification-cost death sentence (most fatal), homogeneous saturation, Sybil and free-riding, governance capture and centralization resurgence, token speculation and compliance risk, scenario inapplicability. These failure modes are not "small problems to note" but fundamental challenges that may cause the whole architecture to fail. This paper's later protocol design and scaling-law hypothesis must both hold under the constraints of these failure modes.

---

## 4 The swarm-intelligence scaling-law hypothesis

### 4.1 The paradigm conversion from single-body scaling to network scaling

The Kaplan scaling law describes the single-body model's loss falling as a power law with compute: L(N) ∝ N^{−α} (α≈0.34) ✅. Chinchilla further corrected the optimal ratio of data to parameters ✅. The common premise of these laws is: **the object of scale is unified, isomorphic, centralized** — one model, one dataset, one hardware cluster.

P2P agent networks break all three premises:

- **Non-unified**: each agent has different models, tools, memory, capabilities, and interests.
- **Non-isomorphic**: nodes' hardware, network, online time, and reputation differ greatly.
- **Non-centralized**: no central scheduler, collaboration spontaneously forms through protocols.

Therefore the single-body scaling law cannot be directly extrapolated. We need a new formalization framework.

**Direct support from the academic frontier:** AgentCollabBench (arXiv:2605.08647, Mazumder et al., 900 manually verified tasks) empirically proves multi-agent reliability is essentially a **network-structure problem** — topology explains 7-40% of information-survival variance, and simply enlarging model intelligence cannot substitute for architecture design ✅ (accessed 2026-09-21). This finding is the direct academic ally of this paper's title argument "a scaling law from agent network structure": the connection topology of the agent network (rather than single-model capability) is an independent dimension determining system-level output quality.

### 4.2 The swarm-intelligence scaling-law formula

This paper proposes the scaling-law hypothesis for swarm-intelligence capability C:

$$C \approx (N_{eff})^{\alpha} \times (D_{collab})^{\beta} \times (V_{eff})^{\gamma} \times (I_{align})^{\delta} \times (G_{trust})^{\varepsilon}$$

The variables are defined as follows:

| Variable | Definition | Dimension/range | Note |
|---|---|---|---|
| C | Swarm-intelligence capability | Task success rate × quality / unit cost | The network's ability to complete effective tasks at unit cost |
| N_eff | Effective node count | Positive integer | Not registered nodes but active nodes continuously contributing and passing the reputation threshold; N_eff ≤ N_registered |
| D_collab | Effective collaboration density | Successful tasks / (coordination cost + verification cost + incentive friction) | Successful collaborations per unit friction cost; P2P's core efficiency metric |
| V_eff | Verification efficiency | Trusted results / verification cost | Trusted results produced per unit verification cost; determines whether verification is economic |
| I_align | Incentive alignment | [0,1] | Consistency of contributors' personal returns with the network's overall returns; 1=fully aligned, 0=fully divergent |
| G_trust | Governance credibility | [0,1] | Fairness, transparency, and anti-capture degree of governance rules; 1=fully credible, 0=fully captured |
| α | Scale exponent | 0 < α < 1 | Sublinear, because coordination and verification costs growing with scale eat part of the returns |
| β,γ,δ,ε | Dimension exponents | >0 | To be empirically calibrated, no specific values preset in this paper [RESULT NEEDED] |

![Figure 1: Agent-network-structure scaling-law non-monotonic curve and paradigm wall](figures/39_fig1_scaling_law_curve.png)

*Figure 1 Conceptual schematic, not measured data. P2P swarm-intelligence capability C varies non-monotonically with N_eff: linear-growth period → sublinear-growth period (α<1) → paradigm-wall period (verification + coordination cost exceeds execution-cost savings). α≈0.6 is the hypothesis preset, to be empirically calibrated [RESULT NEEDED].*

**Key proposition 1 (N_eff ≠ N_registered):** registered node count is a vanity metric. A network with 1 million registered nodes but only 10,000 active contributing nodes has N_eff=10,000, not 1 million. Among the "tens of thousands of miners" projects like Bittensor advertise, the proportion really continuously contributing may be far lower than claimed 🟡.

**Key proposition 2 (α<1, sublinear):** unlike the single-model scaling law, swarm intelligence's scale returns are sublinear. Reason: each added node brings extra coordination cost (discovery, matching, communication) and verification cost (confirming that node's execution is trusted). When these marginal costs exceed marginal returns, adding nodes instead lowers C.

**Key proposition 3 (D_collab and V_eff are decisive variables):** N_eff is only the base, and what really determines C is D_collab and V_eff. A network with N_eff=100,000 but high D_collab (efficient protocols, low-cost verification) may have higher C than one with N_eff=1,000,000 but low D_collab (chaotic coordination, expensive verification). This is the mathematical expression of "protocols matter more than nodes."

### 4.3 Decomposition of effective collaboration density

D_collab is the core efficiency metric this paper proposes, further decomposed as:

$$D_{collab} = \frac{T_{success}}{C_{coord} + C_{verify} + C_{friction}}$$

where:

- T_success: tasks successfully completed per unit time (passing verification and accepted by the demand side).
- C_coord: coordination cost, including task decomposition, routing matching, communication, and retries.
- C_verify: verification cost, including sampling, redundancy, TEE, zkML, and arbitration.
- C_friction: incentive friction, including staking locks, settlement delay, and dispute handling.

**Physical meaning of D_collab:** how many successful tasks each unit spent on coordination + verification + friction produces. This is the P2P network's input-output ratio.

**Comparison with centralized platforms:** the centralized platform's D_central = T_success / C_platform, where C_platform is the platform's operating cost (including data centers, engineers, review). The condition for P2P to hold is D_collab > D_central, i.e. P2P's output per unit friction cost exceeds the centralized platform's output per unit operating cost.

### 4.4 The hardware-decoupling boundary: decoupled from hardware implementation, coupled with hardware constraints

The user material proposed a deep insight: large models let agents not need to solve bottom-layer engineering problems like chips, processes, memory, and storage themselves. This paper rigorously converts it into the **hardware-decoupling boundary proposition**:

> **Proposition:** swarm intelligence is decoupled from hardware implementation (agents need not know whether the bottom layer is GPU, CPU, NPU, or TPU), coupled with hardware constraints (latency, bandwidth, compute, storage, energy consumption remain hard constraints); decoupled from single-point engineering (no need for each node to repeat chip-building), coupled with system cost (the whole network's total compute/bandwidth/energy cost determines the scale ceiling); decoupled from chip process (not caring whether 3nm or 5nm), coupled with physical laws (light-speed latency, energy conservation, bandwidth ceilings cannot be broken).

**Three-layer decoupling:**

1. **Interface decoupling**: agents declare compute/memory/model capabilities through a unified capability declaration (Agent Manifest), not caring about the bottom-layer hardware model.
2. **Supply decoupling**: agents can run across cloud, edge, terminal, and browser, with hardware suppliers replaceable.
3. **Cost decoupling**: agents do not own hardware but can buy or contribute compute on demand.

**Three-layer coupling:**

1. **Physical coupling**: latency (cross-border RTT tens to hundreds of ms), bandwidth (T=Size/Bandwidth), compute (T=Compute/EffectiveFLOPs), storage, energy (E=Power×Time), thermal throttling.
2. **Economic coupling**: unit-task returns must cover compute + verification + coordination + risk costs (see Section 3.1).
3. **Security coupling**: verification, encryption, TEE, zkML all have extra compute and communication overhead.

**Hardware is a multiplier, not an irrelevant item:** the swarm-intelligence formula can be rewritten as:

$$C = \frac{C_{cognitive} \times D_{collab} \times V_{eff} \times I_{align} \times G_{trust}}{C_{coord} + C_{verify} + C_{hardware} + C_{security}} \times H_{bonus}$$

where H_bonus is the hardware-dividend multiplier — stronger edge models, larger memory, faster networks, cheaper storage, cheaper electricity all raise H_bonus and thus C. Hardware progress is not an internal variable of the P2P network, but it is an external multiplier.

**Practical significance of this proposition:** designers of P2P agent networks need not (and should not) solve bottom-layer engineering problems like chip processes, memory bandwidth, and electricity materials — these are the hardware industry's business. But designers must **perceive hardware constraints**: task matching needs to consider nodes' compute/memory/network profiles, latency-sensitive tasks need scheduling to local/edge nodes, and energy-sensitive tasks need scheduling to idle-time/low-price nodes. The hardware-abstraction layer (see Section 5.2) is designed precisely for this.

### 4.5 Falsifiable conditions and forecasts

A scientific hypothesis must be falsifiable. This paper's swarm-intelligence scaling-law hypothesis gives the following clear falsifiable conditions:

**Falsifiable condition 1 (verification-cost death sentence, given in Section 3.1):** if C_verify + C_coord > C_central − C_exec, P2P has no economic viability.

**Falsifiable condition 2 (homogenization death sentence, given in Section 3.2):** if the entropy of agent capability distribution falls over time and task quality does not grow with N_eff, then α→0 and scale expansion has no dividend.

**Falsifiable condition 3 (D_collab falls rather than rises):** if as protocols improve and nodes increase, D_collab does not rise (i.e. coordination + verification costs grow faster than successful task counts), then "P2P scaling" is only a slogan and the hypothesis is falsified.

**Falsifiable condition 4 (N_eff ceiling):** if N_eff stops growing after reaching some threshold (like 100,000) (retention collapse, free-riding too high, Sybil attacks rampant), then the million/billion-level scaling-law extrapolation fails.

**Falsifiable condition 5 (governance capture):** if G_trust falls over time (governance power concentrates, rules manipulated by large nodes), then C's growth is unsustainable and the network finally degenerates to a centralized platform.

**Scoring-time forecast list (design proposal · to implement, [RESULT NEEDED]):**

| Forecast ID | Forecast content | Scoring time | Verification method | Current status |
|---|---|---|---|---|
| F1 | By end-2027, no production-grade P2P agent collaboration network with N_eff>100,000 exists (inference markets like Bittensor do not count as "collaboration networks") | 2027-12-31 | Search public data and industry reports | Pending |
| F2 | The unit cost of P2P AI inference (including verification) will not fall below 50% of centralized cloud APIs before 2028 | 2028-06-30 | Independent cost-comparison tests | Pending |
| F3 | zkML cannot support production-grade inference verification of >7B parameter models before 2028 | 2028-12-31 | Track zkML framework progress | Pending |
| F4 | Any pure P2P (no centralized coordination) agent network has task success rate <80% in latency-sensitive scenarios (<500ms) | 2027-06-30 | Benchmarking | Pending |
| F5 | The median DAO governance voting participation remains <15% over the next 3 years | 2029-09-21 | Track DAO governance data | Pending |
| F6 | Hybrid architecture (centralized coordination + decentralized execution) will become the mainstream form of P2P agent networks, and pure P2P will be eliminated for insufficient performance | 2028-12-31 | Industry architecture observation | Pending |

### Chapter summary

This paper's proposed swarm-intelligence scaling-law hypothesis is falsifiable: C depends on the joint action of N_eff, D_collab, V_eff, I_align, and G_trust, α<1 (sublinear), and D_collab and V_eff are decisive variables. The hardware-decoupling boundary proposition clarifies the design principle of "decoupled from implementation, coupled with constraints." Five falsifiable conditions and six scoring-time forecasts make the hypothesis accept future empirical testing. Currently all exponents (α,β,γ,δ,ε) are [RESULT NEEDED], needing calibration in the UDOS simulation environment or real testnets.

---

## 5 Protocol design: eight-layer stack, four protocol objects, verification hierarchy, and dual-track economy

This chapter is the engineering core of the whole paper. Based on the essence of Chapter 2 and the defect constraints of Chapter 3, it proposes the complete protocol design for P2P agent networks. All designs are marked "design proposal · to implement" + [RESULT NEEDED], not claiming implemented or verified.

### 5.1 The eight-layer protocol stack

| Layer | Name | Core function | Key technologies | Design points |
|---|---|---|---|---|
| L8 | Governance layer | Rule-making, dispute arbitration, protocol upgrades, public-fund allocation | DAO, reputation-weighted voting, delegated voting, expert committees, multi-chamber system | Capital can buy compute but not governance rights; can exit and fork |
| L7 | Settlement layer | Contribution metering, task payments, staking slashing, cross-community settlement | Credit (non-transferable), Token (compliant), fiat/stablecoin, smart contracts | Credit first, Token compliant, no appreciation promise |
| L6 | Verification layer | Execution-result verification, malicious detection, dispute handling | Sampling verification, redundant computation, TEE, zkML, optimistic verification + challenge, arbitration | Five-level hierarchy, choosing strength by task value and privacy |
| L5 | Data layer | Data storage, privacy computing, data authorization and returns | IPFS/content addressing, encrypted storage, federated learning, differential privacy, TEE, zero-knowledge proofs | Data not leaving the domain first, local computing, only returning proofs |
| L4 | Execution layer | Task execution, model inference, tool calls, sandbox isolation | WASM, containers, sandboxes, local models, API models, MCP tool calls | Hardware abstraction, heterogeneous scheduling, power awareness |
| L3 | Communication layer | Agent messaging, streaming, NAT traversal | libp2p, WebRTC, QUIC, MCP/A2A compatible | Async first, relay backstop, not requiring all nodes online simultaneously |
| L2 | Discovery layer | Agent capability registration, task indexing, semantic routing | Kademlia DHT, capability routing, agent registry, sharded subnets, bridge nodes | O(log N) lookup, semantic matching, local markets + cross-domain bridging |
| L1 | Identity layer | Node identity, key management, reputation profiles, contribution history | DID, public keys, verifiable credentials (VC), non-transferable reputation, staking | Identity bound to reputation, reputation non-transferable, anti-Sybil |

**Hardware-abstraction layer (cross-layer):** device-capability registration, task-profile matching, runtime sandboxes, resource metering, execution proofs. The hardware-abstraction layer is not an independent layer but a cross-cutting concern running through L1-L8 — each layer needs to perceive hardware constraints (see Section 4.4).

![Figure 2: P2P agent network eight-layer protocol stack + hardware-abstraction layer (cross-cutting)](figures/39_fig2_protocol_stack.png)

*Figure 2 Conceptual schematic, not measured. The eight-layer protocol stack bottom-up: identity (L1) → discovery (L2) → communication (L3) → execution (L4) → data (L5) → verification (L6) → settlement (L7) → governance (L8); the hardware-abstraction layer is a cross-cutting concern. All layers are design proposals · to implement [RESULT NEEDED].*

**Design principles:**

1. **Not replacing MCP/A2A, but compatible and extending**: the L3 communication layer is compatible with MCP/A2A message formats at the application layer and uses libp2p to replace centralized HTTP at the transport layer.
2. **Async first**: not requiring all agents online simultaneously, tasks can queue and nodes can asynchronously claim (analogous to BOINC's volunteer-computing model).
3. **Local first**: latency-sensitive and privacy tasks scheduled first to local/edge nodes.
4. **Hybrid architecture**: early allowing centralized coordination (the L2 discovery layer can be assisted by centralized indexing services), gradually decentralizing.
5. **Can exit and fork**: any participant can exit the network carrying identity and reputation, and communities can fork the protocol.

### 5.2 The four core protocol objects

The minimal closed loop of the protocol consists of four objects: **Agent Manifest → Task Envelope → Receipt → Reputation**. These four objects correspond to the four stages of the task lifecycle: discovery → execution → verification → settlement.

![Figure 3: Agent magnet-link task closed-loop sequence: four protocol objects and nine-step lifecycle](figures/39_fig3_task_lifecycle.png)

*Figure 3 Conceptual schematic, not measured. The four protocol objects (Agent Manifest/Task Envelope/Receipt/Reputation) correspond to the four stages of the task lifecycle; the nine-step closed loop: discovery → publish → match → take → submit → verify → settle → reputation update → cycle. The JSON schema is a design draft · to implement [RESULT NEEDED].*

#### 5.2.1 Agent Manifest (agent magnet link)

The Agent Manifest is the agent's capability declaration and identity card, content-addressed as `agent://<cid>?v=1`, like a BitTorrent magnet link. Anyone obtaining this Manifest can discover, call, assign tasks, provide compute, verify results, and share/seeding.

```json
{
  "agent_id": "did:key:z6Mk...",
  "version": "1.0",
  "name": "translation-zh-en",
  "capabilities": ["translate", "summarize", "code_review"],
  "io_schema": {
    "input": {"type": "text", "max_tokens": 8192},
    "output": {"type": "text", "max_tokens": 8192}
  },
  "model_hashes": ["sha256:abc123..."],
  "runtime": ["wasm", "docker", "browser", "api"],
  "hardware_profile": {
    "min_memory_gb": 8,
    "preferred_compute": ["gpu", "npu", "cpu"],
    "max_latency_ms": 5000
  },
  "tools": ["ocr", "glossary", "web_search"],
  "pricing": {
    "unit": "1k_tokens",
    "amount": "0.01",
    "currency": "credit"
  },
  "verification": {
    "default_mode": "sample",
    "supported_modes": ["sample", "redundant", "tee", "zkml"],
    "challenge_window": "1h"
  },
  "stake": {"amount": 100, "currency": "credit"},
  "reputation": "did:key:z6Mk.../rep",
  "endpoints": ["libp2p://Qm...", "https://..."],
  "dht_key": "sha256:def456...",
  "settlement": "chain:0x...",
  "license": "Apache-2.0",
  "privacy_policy": "data_not_stored",
  "signature": "..."
}
```

**Key-field notes:**

- `agent_id`: DID identity, bound to the public key, unforgeable.
- `capabilities` + `io_schema`: capability declaration, used for DHT semantic routing and task matching.
- `model_hashes`: declares the model-weight hashes used, and verifiers can confirm execution used the declared model.
- `hardware_profile`: hardware profile (see Section 4.4), used for power-aware scheduling.
- `verification`: declares supported verification modes, and task publishers can choose appropriate verification strength.
- `stake`: staking amount, slashed on malicious acts.
- `reputation`: reference pointing to the reputation profile, reputation non-transferable.

#### 5.2.2 Task Envelope

The Task Envelope is the complete definition of the task, including input, verification strategy, bounty, privacy requirements, and deadline.

```json
{
  "task_id": "cid:task789...",
  "type": "translation",
  "input_cid": "cid:bafy...",
  "input_hash": "sha256:...",
  "output_requirements": {
    "type": "text",
    "quality_bar": "human_parity",
    "max_tokens": 4096
  },
  "verification": {
    "mode": "redundant",
    "redundancy": 3,
    "consensus_threshold": "2_of_3",
    "challenge_window": "24h"
  },
  "privacy": {
    "level": "local_only",
    "data_retention": "delete_after_verification",
    "allowed_regions": ["CN", "SG"]
  },
  "budget": {
    "execution_fee": 10,
    "verification_fee": 3,
    "protocol_fee": 0.5,
    "currency": "credit"
  },
  "deadline": "2026-09-22T00:00:00Z",
  "shard": {"total": 1, "index": 0},
  "publisher": "did:key:z6Mk...",
  "signature": "..."
}
```

**Isomorphic relation with UDOS Paper 09's TransferBundle:** the Task Envelope is isomorphic to the TransferBundle's six fields (Goal/Context/Done/Todo/Trace/Owner) — Goal→task definition, Context→input CID and privacy requirements, Done→output requirements and verification strategy, Todo→execution and deadline, Trace→resource metering and Receipt, Owner→publisher signature.

#### 5.2.3 Receipt

The Receipt is the proof of the execution result, including result hash, resource metering, signature, and optional TEE/zk proofs.

```json
{
  "receipt_id": "cid:receipt012...",
  "task_id": "cid:task789...",
  "executor": "did:key:z6Mk...",
  "result_hash": "sha256:result...",
  "result_cid": "cid:bafyresult...",
  "resource_metering": {
    "compute_flops": 1.5e12,
    "memory_peak_gb": 6.2,
    "network_bytes_in": 10240,
    "network_bytes_out": 20480,
    "duration_ms": 3200,
    "energy_wh": 0.0045
  },
  "model_used": "sha256:abc123...",
  "proof": {
    "type": "tee_attestation",
    "evidence": "base64...",
    "verified_by": "did:key:verifier..."
  },
  "verification_result": {
    "mode": "redundant_2_of_3",
    "passed": true,
    "verifiers": ["did:key:v1...", "did:key:v2...", "did:key:v3..."],
    "dispute": null
  },
  "timestamp": "2026-09-21T12:00:00Z",
  "signature": "..."
}
```

**Resource metering is the basis of settlement:** the resource_metering field in the Receipt precisely records the compute, memory, network, time, and energy consumed by execution, the basis of contribution metering and settlement. This solves the core problem of "contributions not measurable" in volunteer computing.

#### 5.2.4 Reputation

Reputation is the multidimensional reputation aggregated from Receipts, non-transferable, determining priority, staking quota, and governance weight.

```json
{
  "agent_id": "did:key:z6Mk...",
  "dimensions": {
    "quality": 0.92,
    "speed": 0.85,
    "honesty": 0.98,
    "availability": 0.78
  },
  "aggregate": 0.89,
  "history": {
    "total_tasks": 1843,
    "successful": 1712,
    "disputed": 23,
    "slashed": 5
  },
  "decay": {
    "half_life_days": 90,
    "last_update": "2026-09-21"
  },
  "stake_amount": 100,
  "governance_weight": 0.89,
  "signature": "..."
}
```

**Design points:**

- **Multidimensional reputation**: four dimensions of quality, speed, honesty, availability, and different tasks can weight different dimensions (e.g. high-value tasks weight honesty and quality, batch tasks weight speed and availability).
- **Non-transferable**: reputation bound to the DID identity, not buyable/sellable. This prevents capital from obtaining governance rights by buying reputation — "capital can buy compute, but cannot buy reputation."
- **Decay mechanism**: reputation decays over time (90-day half-life), preventing early contributors from "lying flat" on old credit and also reducing the long-term returns of Sybil reputation farming.
- **Staking association**: the higher the reputation, the lower the required staking (because malicious probability is low); the lower the reputation, the higher the required staking. This aligns reputation with economic incentives.

### 5.3 Five-level verification hierarchy

Based on the analysis in Section 2.3 (zkML performance poor, TEE hardware root of trust, optimistic verification latency high), no single verification scheme covers all scenarios. This paper proposes a five-level verification hierarchy, choosing appropriate strength by task value and privacy requirements:

| Level | Name | Verification method | Verification overhead | Finality delay | Applicable scenarios | Malicious cost |
|---|---|---|---|---|---|---|
| L0 | Self-report + sampling | Executor self-reports results, randomly sampling 5-10% to re-verify | Extremely low (5-10% execution cost) | Immediate (after sampling) | Low-value, low-risk tasks (translation, summary, annotation) | Sampling-detection probability × slashing |
| L1 | Redundant 2-of-3 | 3 independent nodes execute, 2/3 agreement passes | Medium (about 200% execution cost) | Minutes (waiting for 3 nodes) | Medium-value tasks (code review, data analysis, document Q&A) | Need to control 2/3 nodes |
| L2 | TEE trusted execution | Executor executes in the TEE, submits remote attestation | Low (TEE performance overhead 5-15%) | Immediate (attestation verification fast) | Privacy-sensitive tasks (medical, financial, personal data) | Need to breach TEE side channels |
| L3 | zkML + optimistic challenge | Executor submits zk proof, sets challenge window | High (zk proof generation 100-1000× execution time) | Challenge window (hours to days) | High-value, formally verifiable tasks | Need to breach the zk proof system |
| L4 | Committee arbitration + staking slashing | Multisig committee reviews disputes, staking-slashes malicious actors | High (human/committee review) | Days to weeks | Disputed tasks, high-value disputes, protocol upgrades | Reputation destruction + economic slashing |

**Economic model of verification cost:** the choice of verification strength should satisfy:

$$C_{verify}(level) \leq V_{task} \times P_{fraud}$$

i.e. verification cost does not exceed task value times fraud probability. For a $0.01 translation task, L0 sampling suffices; for a $10,000 medical-diagnosis task, L2 TEE + L4 arbitration is needed.

**Mapping with UDOS Paper 06:** the five-level verification hierarchy is isomorphic to Paper 06's three-level typed finality (probabilistic / economic / deterministic) — L0 corresponds to probabilistic finality (sampling passes means probably correct), L1-L3 to economic finality (malicious cost > return means rational non-malice), L4 to deterministic finality (irreversible after committee arbitration).

### 5.4 Dual-track economic model: credit, Token, and fiat

**Three-track separation principle (compliance red line):**

| Track | Nature | Transferability | Use | Compliance requirement |
|---|---|---|---|---|
| Credit | Internal contribution-accounting unit | Non-transferable/non-tradeable | Reputation, quotas, priority, staking | None (no monetary value) |
| Token (if needed) | Cross-community settlement and staking | Restricted transfer | Cross-community settlement, staking, governance | Compliant issuance, no appreciation promise, not investment advice |
| Fiat/stablecoin | Real payment | Fully transferable | Enterprise procurement, wages, tax | KYC/AML, licensed channels |

**Why is credit non-transferable?** If credit were transferable, it would have monetary value and might be determined by regulators as a security or virtual currency, triggering compliance risk. Non-transferable credit is purely a "contribution proof," like airline miles (the non-transferable version), used to raise service levels rather than trade.

**Contribution-metering formula (design proposal · to implement):**

$$Contribution = \sum_{i} (Resource_i \times Time_i \times Utilization_i \times Quality_i \times Scarcity_i) - Penalty$$

where:

- Resource_i: the resource amount contributed (compute FLOPs, storage GB, bandwidth MB, data volume, feedback count).
- Time_i: contribution duration.
- Utilization_i: resource utilization (preventing false reporting of idle but actually not contributing).
- Quality_i: quality coefficient (from Reputation's quality dimension, 1 if Receipt verification passes, 0 or negative if fails).
- Scarcity_i: scarcity coefficient (scarce resources like GPU compute have higher coefficients than abundant resources like CPU compute).
- Penalty: punishment (malicious slashing, task failure, dispute loss).

**Distribution principle:** automatically settle by Receipt; disputed tasks delay settlement until arbitration ends; the public pool (a certain proportion of protocol fees, suggested 10%) is used for cold-start subsidies, security audits, and public goods (open-source models, datasets, verification tools).

**Anti-capital-monopoly design:**

- Capital can buy compute (contributing more Resource_i), but cannot buy Quality_i (quality comes from historical performance) and Reputation (non-transferable).
- Governance weight is based on Reputation (reputation weighting) rather than Token holdings (token weighting), preventing whales from controlling governance.
- Delegated voting lets small contributors delegate governance to trusted representatives, but representatives must periodically disclose voting records.
- Multi-chamber governance: contributor committee (reputation-weighted) + expert committee (technical capability) + user committee (user interests) + public-fund oversight committee, with any protocol upgrade needing multi-chamber co-signature.

### 5.5 Role differentiation: not everyone runs large models

Nodes in P2P agent networks are not homogeneous but differentiated into different roles:

| Role | Contribution | Typical hardware | Return source | Ordinary-person participation |
|---|---|---|---|---|
| Compute node | Inference, verification, storage | Phones, PCs, edge devices | Compute-contribution credit | High (idle-time computing while phone charging) |
| Model node | Expert models, tools, algorithms | GPU servers | Model-use fees | Medium (needs technical capability) |
| Data node | Private data (federated learning + differential privacy + TEE) | Phone/PC | Data-authorization returns | High (data not leaving the domain) |
| Agent node | Personal agent, negotiating/executing/collaborating on one's behalf | Phone/PC | Task-execution fees | High (everyone has an agent) |
| Verification node | Sampling audits, redundant computation, challenge arbitration | PC/server | Verification fees | Medium (needs reputation and staking) |
| Routing node | Task decomposition, matching, cross-subnet bridging | Servers | Routing fees | Low (needs technical capability) |
| Governance node | Rule-making, dispute arbitration, public funds | No special hardware | Governance stipends | Medium (needs high reputation) |
| Compliance/insurance node | Legal entities, insurance pools, audits, liability isolation | No special hardware | Service fees | Low (needs legal/financial qualification) |

**Key insight: the core of ordinary people's contribution is not idle compute.** The market value of idle compute is very low — the marginal cost of global GPU compute is priced by centralized cloud, and P2P idle compute is hard to compete on price. Ordinary people's truly scarce contributions are:

1. **Long-tail scenarios and local knowledge**: localized, verticalized, real-time scenarios centralized platforms cannot cover (like community Q&A, local experience, minority-language translation).
2. **Data sovereignty**: the authorized use of personal data under privacy protection, which centralized platforms cannot legally obtain.
3. **Human feedback**: annotation, evaluation, correction, preferences — these are core resources of RLHF and model evaluation and cannot be replaced by compute.
4. **Verification and governance**: human review, dispute arbitration, community governance — these need human judgment and cannot be automated.
5. **Social trust**: acquaintance networks, community reputation, guarantee relationships — this is the social capital of anti-Sybil.

### Chapter summary

The eight-layer protocol stack (identity → governance) defines the complete architecture of P2P agent networks; the four protocol objects (Agent Manifest/Task Envelope/Receipt/Reputation) form the minimal closed loop of the task lifecycle; the five-level verification hierarchy chooses verification strength by task value and privacy; the dual-track economy (non-transferable credit + compliant Token + KYC fiat) achieves incentive alignment within compliance red lines; role differentiation clarifies that ordinary people's core contribution is not idle compute but long-tail scenarios, data sovereignty, feedback, verification, and social trust. All designs are "design proposals · to implement" [RESULT NEEDED].

---

## 6 Mapping with the existing UDOS system

This chapter establishes the precise mapping of P39 with the 13 papers of the UDOS v7.7.2 master volume, ensuring the supplementary volume does not contradict the existing system but extrapolates and deepens on its basis.

### 6.1 Paper-by-paper mapping table

| UDOS paper | Theme | P39's mapping and extrapolation | Relation |
|---|---|---|---|
| Paper 02 | Structure×scale scaling law, three-segment curve, paradigm wall | P39 supplements "network structure" as the decentralized extrapolation of T (connection topology) in the structural triplet S=(T,G,M); the distinction between N_eff and the scale axis N; P2P's "paradigm wall" = verification-cost wall | Extrapolation and deepening |
| Paper 04 | RSI convergence path, L1-L5 lineage | Whether P2P networks constitute a mass line of "structural self-modification" — agent networks self-reorganizing topology = distributed L4/L5? Cautious judgment: no existing P2P network reaches RSI self-modification, marked [RESULT NEEDED] | Cautious extrapolation |
| Paper 05 | Coordination topology, hierarchical topology, communication protocols, memory wall, million-level extrapolation | P2P's DHT/semantic routing/sharded subnets = decentralized implementation of fan-in O(log N); local markets + bridge agents = hierarchical hybrid tree; Paper 05's memory-complexity dimension (Warp-Cortex O(N·L)→O(1) weights) corresponds in P2P to distributed sharding of state storage | Direct extrapolation |
| Paper 06 | Consensus stopping, BFT-lite, three-level typed finality, rules first | Receipt + layered verification = decentralized BFT-lite; verification cost = marginal cost of the 2f+1 quorum; five-level verification (L0 sampling → L4 arbitration) is isomorphic to three-level finality (probabilistic/economic/deterministic); The Cost of Consensus's sycophantic compliance is the theoretical basis of P2P homogeneous saturation | Direct extrapolation |
| Paper 07 | Quality saturation law, heterogeneity dividend, simulation validity | Isomorphic copies bring no gain — P2P node heterogeneity/independent information sources are the dividend; Bittensor-style homogeneous subnets = homogeneous-expansion saturation; the quality saturation law mse(k)=a+b/k directly applies to choosing the optimal copy count for P2P redundant verification | Core theoretical basis |
| Paper 09 | Handoff contract, TransferBundle, MCP/A2A interop | Task Envelope/Receipt is isomorphic to the TransferBundle six fields (Goal→task definition, Context→input CID, Done→output requirements, Todo→execution and deadline, Trace→resource metering, Owner→signature); the docking of Agent Manifest with A2A Signed Agent Card; the adaptation of MCP stateless core at the P2P transport layer | Protocol-object isomorphism |
| Paper 10 | Agent task market, settlement conservation, Byzantine slashing, VCG, settlement neutrality | UDOS's internal task market (budget/slashing/conservation balance_sum=total_paid) directly extrapolates to the P2P settlement market; open market vs closed accounting; the application of VCG-style payments in P2P task bidding; the challenge of settlement neutrality in decentralized settlement | Direct extrapolation |
| Paper 13 | Evidence engineering, evidence grading, literature ledger | Figures of token projects like Bittensor/io.net uniformly three-state; not endorsing any project; P39's EVIDENCE_LEDGER follows Paper 13's evidence-grading standards | Methodological consistency |

### 6.2 Systematic dialogue with Paper 02's scaling law

Paper 02 established the structure×scale three-segment scaling-law curve: linear growth → sublinear growth → paradigm wall (verified, tracing reports7/). P39's swarm-intelligence scaling-law hypothesis is the concrete unfolding of this framework on the "network-structure axis":

- **First segment (linear growth)**: when N_eff is small (<1000), coordination and verification costs are low, and adding nodes approximately linearly raises C.
- **Second segment (sublinear growth)**: when N_eff grows to 10,000-100,000, coordination and verification costs begin eating scale returns, α<1.
- **Third segment (paradigm wall)**: when N_eff exceeds some threshold, the growth of verification + coordination costs exceeds execution-cost savings, and C peaks or even falls. This threshold is P2P's "paradigm wall" — determined not by model capability but by protocol efficiency (D_collab and V_eff).

**Key distinction:** Paper 02's paradigm wall is mainly determined by hardware/memory/communication physical constraints; P39's paradigm wall is mainly determined by protocol efficiency and verification cost. This means P2P's paradigm wall is **movable through protocol improvement** — if D_collab and V_eff rise with protocol improvement, the paradigm-wall threshold can be raised. This is precisely the meaning of falsifiable condition 3 (if D_collab falls rather than rises, the hypothesis is falsified).

### 6.3 Cautious dialogue with Paper 04's RSI

Paper 04 established RSI's L1-L5 lineage (verified). A natural question is: do P2P agent networks constitute "distributed RSI"?

**Cautious judgment:**

- **L1-L2 (tool use/parameter self-tuning)**: a single agent in a P2P network can reach L1-L2, but this is not network-level RSI.
- **L3 (architecture self-modification)**: if the P2P network's protocol itself can automatically upgrade through governance voting (like a DAO voting through new verification rules and automatically deploying), this may constitute network-level L3. But currently no P2P network has implemented this [RESULT NEEDED].
- **L4-L5 (goal self-modification/complete self-evolution)**: the P2P network's goals are determined by governance, and if governance can modify governance rules themselves (amending the constitution), this may constitute L4. But this brings huge security risk — "modifying the rules that modify rules" may cause value drift and uncontrollable evolution. Paper 04 has warned of RSI's convergence risk, and P2P networks' distributed RSI risk is greater (no single entity can emergency-stop).

**Conclusion:** the relation of P2P agent networks to RSI is "potential carrier" rather than "implemented." Currently all P2P networks fail to reach network-level RSI, marked [RESULT NEEDED]. In design, the P2P network's governance layer should set "constitutional rigid clauses" — certain core rules (like privacy protection, exit rights, anti-Sybil mechanisms) cannot be modified by ordinary governance votes, needing higher thresholds (like a full referendum + delayed effectiveness + rollback mechanisms) to prevent value drift.

### Chapter summary

P39 forms a precise mapping with the 13 UDOS papers: Paper 02 provides the scaling-law framework, Paper 05 the coordination-topology basis, Paper 06 the consensus and verification theory, Paper 07 the core basis of quality saturation and heterogeneity dividends, Paper 09 the isomorphism basis of protocol objects, Paper 10 the task-market and settlement theory, Paper 13 the evidence methodology. The relation to Paper 04's RSI is a cautious "potential carrier" rather than implemented. P39 is not a new paper independent of the UDOS system but the extrapolation and deepening of the existing system on the P2P agent-network dimension.

---

## 7 Cold-start path and hybrid architecture

### 7.1 Why not start with a "global billion-level P2P"

P2P networks have the classic cold-start problem: network value is proportional to the square of node count (Metcalfe's law), but early when nodes are few network value is low, unable to attract new nodes. Moreover:

- **Pure P2P performance unreachable**: as described in Section 3.6, pure P2P is far inferior to centralization in latency, reliability, and SLA. If starting with pure P2P, user experience is poor, retention low, and the network cannot grow.
- **Verification infrastructure missing**: the five-level verification hierarchy needs infrastructure like TEE, zkML, and arbitration committees, impossible to all be ready early.
- **Governance mechanisms need running-in**: DAO governance, reputation systems, and dispute arbitration need to iterate in practice and cannot be perfect from the start.
- **Compliance risk**: introducing Token and cross-border settlement early makes compliance complexity extremely high and may cause the project to be halted by regulators.

Therefore, **the hybrid architecture (centralized coordination + decentralized execution) is the only engineering-feasible cold-start path**.

### 7.2 Six-stage cold-start roadmap

| Stage | Name | Node scale | Architecture | Core task | Key milestone | Decentralization degree |
|---|---|---|---|---|---|---|
| 0 | Single-machine client | 1 | Pure centralization | Personal agent management, running models locally, connecting APIs | Client usable, local inference working | 0% (pure centralization) |
| 1 | Invitation-only small community | 100-1000 | Centralized coordination + decentralized execution | Sharing compute/models/agents, running credit, human verification | 100-node closed loop, task success >80% | 10% (execution decentralization) |
| 2 | Agent magnet links | 1000-10000 | Centralized indexing + P2P discovery | Agent Manifest standardization, DHT discovery, one-click share/run/seeding | 1000 agents discoverable and callable | 30% (discovery decentralization) |
| 3 | Vertical task market | 10,000-100,000 | Centralized matching + P2P execution + automatic verification | Task publishing/taking/bidding, sampling verification, reputation, credit settlement | Daily 10,000 tasks, verification cost <50% execution | 50% (execution + verification decentralization) |
| 4 | Cross-platform protocols | 100,000-1,000,000 | Sharded DHT + local markets + bridge nodes | MCP/A2A compatible, federated subnets, Token/stablecoin settlement, dispute arbitration | Cross-3-community interop, settlement automated | 70% (network + settlement decentralization) |
| 5 | Hybrid open network | 1,000,000-1,000,000,000 | Centralized coordination optional + full-stack decentralization | Sharded DHT million agents, DAO governance, public funds, insurance compliance | N_eff>100,000, governance participation >15% | 90% (governance decentralization, coordination still optionally centralized) |

![Figure 4: P2P agent network six-stage cold-start roadmap and hybrid architecture](figures/39_fig4_cold_start_roadmap.png)

*Figure 4 Conceptual schematic, not measured. Six stages: single-machine client (0%) → invitation-only small community (10%) → agent magnet links (30%) → vertical task market (50%) → cross-platform protocols (70%) → hybrid open network (90%). The hybrid architecture is a permanent choice (centralized coordination optional + P2P network layer + local execution layer), not an expedient. Each stage's scale and decentralization degree are design targets · to implement [RESULT NEEDED].*

**Core-scenario entry for each stage:**

- **Stages 0-1**: personal knowledge management, local document Q&A, lightweight agent assistants (not relying on the network).
- **Stages 2-3**: translation, data annotation, code review, content moderation, simulation compute (splittable, verifiable, latency-insensitive).
- **Stages 4-5**: scientific reproduction, long-tail knowledge Q&A, federated learning, multi-agent collaboration, decentralized AI service markets.

### 7.3 Design principles of the hybrid architecture

The hybrid architecture is not an expedient of "temporarily centralized, future decentralized" but a **permanent architecture choice**. Even at stage 5, centralized coordination remains optional — for low-latency, high-SLA tasks users can choose a centralized coordinator (paying); for cost-sensitive, latency-insensitive tasks users can choose pure P2P.

**Three layers of the hybrid architecture:**

1. **Centralized coordination layer (optional)**: task indexing, matching, scheduling, quality monitoring. Operated by a foundation or compliant company, charging service fees. Users can choose to use or skip.
2. **P2P network layer**: agent discovery, communication, execution, verification, settlement. Fully decentralized, no single point of failure.
3. **Local execution layer**: agent runtime, local models, private data on personal devices. Fully local, data not leaving the domain.

**Why is pure P2P not the goal?** As described in Chapter 3, pure P2P has inherent defects in performance, reliability, and SLA. The hybrid architecture acknowledges these defects, uses centralized coordination to make up P2P's shortcomings, and uses P2P to make up centralization's monopoly and censorship risks. **The goal is not "decentralization" but "choice"** — users can freely choose between centralization and decentralization according to task needs.

### 7.4 Key metrics and success criteria

| Metric | Definition | Stage 3 target | Stage 5 target | Failure criterion |
|---|---|---|---|---|
| N_eff | Active nodes continuously contributing and passing the reputation threshold | >1000 | >100,000 | <5% of registered nodes |
| Task success rate | Proportion of tasks passing verification and accepted by the demand side | >85% | >95% | <70% |
| Verification-cost ratio | C_verify / C_exec | <50% | <20% | >100% (verification death sentence) |
| Unit task cost | End-to-end cost including verification + coordination | <150% of centralized API | <80% of centralized API | >200% of centralized API |
| Free-riding rate | Proportion of nodes only consuming not contributing | <40% | <25% | >70% |
| Governance participation | Participation in governance voting | >5% | >15% | <1% (governance capture) |
| Ordinary-person return share | Proportion of returns obtained by non-institutional nodes | >30% | >50% | <10% (centralization resurgence) |
| Privacy-incident rate | Data-leak/privacy-violation events per quarter | <1 | 0 | >5/quarter |

**Core success criterion:** at stage 5, verification-cost ratio <20% and unit task cost <80% of centralized API and ordinary-person return share >50%. If these three conditions simultaneously hold, the P2P agent network's economic rationality and the "mass line's" value promise simultaneously hold. If any condition fails, return to protocol-design improvement or acknowledge P2P inapplicable in that scenario.

### Chapter summary

Cold start adopts the six-stage roadmap (single machine → small community → magnet links → task market → cross-platform → hybrid open network), with decentralization degree gradually rising. The hybrid architecture is a permanent choice rather than an expedient — centralized coordination makes up P2P's performance defects, P2P makes up centralization's monopoly risk, and the goal is "choice" rather than "pure decentralization." Eight key metrics define the success criteria, among which verification-cost ratio, unit task cost, and ordinary-person return share are core.

---

## 8 Risks, failure boundaries, falsifiable conditions, and forecasts

This chapter aggregates the whole paper's risk list, failure boundaries, falsifiable conditions, and forecasts, the paper's critical closure.

### 8.1 Risk panorama table

| Risk ID | Risk | Severity | Likelihood | Impact | Mitigation | Residual risk |
|---|---|---|---|---|---|---|
| R1 | Verification-cost death sentence | Fatal | Medium | P2P no economic viability, architecture fails | Five-level layered verification, L0 sampling first, only high-value tasks use high-cost verification | Medium (generative-task verification essentially hard) |
| R2 | Homogeneous saturation | High | High | Scale expansion no dividend, α→0 | Incentivize heterogeneity, reward independent information sources, punish convergence | Medium (verification mechanisms may naturally incentivize convergence) |
| R3 | Sybil attack | High | High | Reputation/governance/matching manipulated | DID + non-transferable reputation + staking + Proof of Personhood + behavior analysis | Medium (no perfect anti-Sybil scheme) |
| R4 | Free-riding/tragedy of the commons | High | High | Supply side shrinks, network degenerates | Contribution metering + credit consumption + priority tilt + staking | Medium (free-riding is P2P's inherent problem) |
| R5 | Governance capture/whale control | High | Medium | Decentralization promise bankrupt, degenerates to centralization | Reputation weighting rather than token weighting + multi-chamber system + delegated voting + constitutional rigid clauses | Medium (information asymmetry makes elite control hard to avoid) |
| R6 | Centralization resurgence | High | High | Entry/compute/model/governance re-centralized | Can exit and fork + multiple clients + open protocols + anti-monopoly design | High (network effects naturally favor centralization) |
| R7 | Token speculation/compliance risk | High | Medium | Project halted by regulators, community split | Credit first, Token compliant, no appreciation promise, KYC/AML | Medium (regulatory environment uncertain) |
| R8 | Privacy leak/data security | High | Medium | User data leaked, legal liability | Local first + encryption + differential privacy + TEE + data not leaving the domain | Medium (TEE has side channels, federated learning has gradient leaks) |
| R9 | Prompt injection/data poisoning/model theft | Medium | High | Execution results untrusted, model IP lost | Sandbox isolation + input filtering + redundant verification + model watermarking + reputation punishment | Medium (agent security is an open question) |
| R10 | Performance inferior to centralization | Medium | High | User experience poor, retention low | Hybrid architecture + local first + edge caching + async first | High (P2P's physical latency cannot be eliminated) |
| R11 | Smart-contract security/fund loss | Medium | Medium | Settlement funds stolen | Formal verification + audits + insurance pools + gradual decentralization | Low (mitigable through mature schemes) |
| R12 | Distributed RSI out of control | Fatal | Low | Network-level self-evolution uncontrollable | Constitutional rigid clauses + emergency-stop mechanisms + multisig governance + rollback | Low (current technology far from reaching, but needs pre-design) |

### 8.2 Failure-boundary summary

This paper's propositions and designs hold within the following boundaries, and beyond the boundaries they are inapplicable or need correction:

1. **Scenario boundary**: P2P applies to batch, splittable, verifiable, latency-insensitive tasks; not to low-latency real-time inference, large-model pretraining, strong privacy sensitivity, high-frequency trading, or enterprise services needing SLA (see Section 3.6).
2. **Scale boundary**: the swarm-intelligence scaling law's α<1 holds when N_eff<1 million; whether it still holds when N_eff>1 million [RESULT NEEDED], with new paradigm walls possibly appearing.
3. **Verification boundary**: the five-level verification hierarchy applies to tasks formally or redundantly verifiable; for purely subjective generative tasks (like artistic creation, literary writing), verification is essentially hard and L4 arbitration may be the only scheme.
4. **Economic boundary**: P2P's economic rationality rests on C_verify + C_coord < C_central − C_exec; if centralized API prices keep falling (like inference cost falling 50% annually 🟡), P2P's cost advantage may be compressed.
5. **Governance boundary**: multi-chamber governance is effective when participants have basic technical literacy and information access; under severe information asymmetry or participant apathy, governance may degenerate to elite control.
6. **Compliance boundary**: the feasibility of Token and cross-border settlement depends on jurisdictions' regulatory policy; in jurisdictions strictly banning virtual currency, only credit and fiat channels can be used.
7. **Hardware boundary**: P2P network performance is limited by nodes' hardware level; in scenarios with a high proportion of mobile nodes, total compute and network quality may be insufficient to support complex tasks.
8. **RSI boundary**: the relation of P2P networks to RSI is "potential carrier," and currently no P2P network reaches network-level RSI; distributed RSI risk is greater than single-body RSI (no single entity can emergency-stop), needing pre-set constitutional rigid clauses.

### 8.3 Falsifiable-condition summary

This paper's core hypothesis (swarm-intelligence scaling law) gives five falsifiable conditions (see Section 4.5), summarized as follows:

1. **Verification-cost death sentence**: if C_verify + C_coord > C_central − C_exec, P2P has no economic viability.
2. **Homogenization death sentence**: if the entropy of agent capability distribution falls over time and task quality does not grow with N_eff, then α→0.
3. **D_collab falls rather than rises**: if as protocols improve and nodes increase D_collab does not rise, "P2P scaling" is only a slogan.
4. **N_eff ceiling**: if N_eff stops growing after some threshold (retention collapse, free-riding too high, Sybil rampant), the million/billion-level extrapolation fails.
5. **Governance capture**: if G_trust falls over time, C's growth is unsustainable and the network degenerates to a centralized platform.

**Moreover, this paper's design proposals (eight-layer stack, four protocol objects, five-level verification, dual-track economy) are all falsifiable engineering propositions**: if after implementation in a testnet the key metrics (task success rate, verification-cost ratio, unit task cost, ordinary-person return share) do not meet targets, the design needs correction or abandonment.

### 8.4 Forecast list (scoring time)

Six scoring-time forecasts were given in Section 4.5 (F1-F6). Three long-term forecasts are added here:

| Forecast ID | Forecast content | Scoring time | Verification method |
|---|---|---|---|
| F7 | By 2030, P2P agent networks will occupy >10% market share in at least one vertical scenario (like data annotation or translation) | 2030-12-31 | Industry market reports |
| F8 | By 2030, no production-grade P2P agent network fully decentralized (without any centralized component) exists; the hybrid architecture will be the only form | 2030-12-31 | Industry architecture observation |
| F9 | By 2032, the swarm-intelligence scaling law's α exponent will be empirically calibrated in at least one real P2P network, with 0.3<α<0.7 | 2032-12-31 | Academic papers or industry white papers |

### Chapter summary

12 risks (including 2 fatal), 8 failure boundaries, 5 falsifiable conditions, and 9 scoring-time forecasts together form this paper's critical closure. P2P agent networks are not a universal solution and have clear failure boundaries on eight dimensions: scenario, scale, verification, economy, governance, compliance, hardware, and RSI. Honestly facing these boundaries is the premise of engineering-oriented argument and also the requirement of "taking the essence and removing the defects."

---

## Conclusion: the mass line is not a slogan but an engineering proposition

The core conclusions of this paper can be summarized in three sentences:

**First, the bottleneck of million-level agents is coordination rather than single-body intelligence.** When agent counts enter the million-to-billion level, the five walls the system meets (coordination, trust, verification, incentives, governance) are all organizational problems rather than model-capability problems. The large-model scaling law cannot be directly extrapolated to agent networks and needs a new formalization framework.

**Second, the scaling law of P2P agent networks is a falsifiable engineering hypothesis, not a revolutionary slogan.** This paper proposes C ≈ (N_eff)^α × (D_collab)^β × (V_eff)^γ × (I_align)^δ × (G_trust)^ε (α<1), clarifies the distinction between N_eff and registered node count, emphasizes the decisive role of D_collab and V_eff, and gives five falsifiable conditions. If verification cost exceeds centralized recomputation, if agent homogenization causes saturation, if D_collab does not rise with protocol improvement, then "P2P scaling" is falsified.

**Third, the value promise of the "mass line" must land on verifiable mechanism design.** Participation descent needs low-threshold light nodes, return descent needs verifiable contribution metering and the dual-track economy (non-transferable credit + compliant Token + KYC fiat), governance descent needs Sybil-resistant reputation systems and multi-chamber governance. The eight-layer protocol stack, four protocol objects, five-level verification hierarchy, and six-stage cold-start roadmap are the concrete designs of these mechanisms (all design proposals · to implement [RESULT NEEDED]). The hybrid architecture (centralized coordination + decentralized execution) is the only engineering-feasible path, and pure P2P is unreachable for performance defects.

**Contribution to the UDOS system:** P39 extrapolates Paper 02's scaling-law framework to the network-structure axis, Paper 05's coordination topology to DHT/semantic routing/sharded subnets, Paper 06's consensus verification to the five-level verification hierarchy, applies Paper 07's quality saturation law to P2P homogenization critique, makes Paper 09's TransferBundle isomorphic to Task Envelope/Receipt, and extrapolates Paper 10's task market to the open P2P settlement market. P39 is not an independent new paper but the complete extrapolation and deepening of the UDOS system on the P2P agent-network dimension.

**Final judgment:** P2P agent networks have a real technical basis (libp2p, DHT, MCP/A2A, TEE, federated learning), real demand (anti-monopoly, data sovereignty, long-tail scenarios, inclusive AI), but also real fatal risks (verification-cost death sentence, homogeneous saturation, Sybil attacks, governance capture, centralization resurgence). It is not a revolution bound to succeed but an engineering proposition needing gradual verification in testnets, possibly succeeding in some scenarios and failing in others.

**The true meaning of the "mass line" is not "everyone runs large models" but "everyone owns an agent, everyone can contribute, verify, govern, and share the returns of intelligence."** Whether this vision can be realized depends not on how loud the slogan is, but on whether protocols can press verification cost low enough, raise collaboration density high enough, and truly keep governance power in ordinary people's hands. This is a falsifiable engineering proposition; let us see the outcome in testnets.

---

## References

### Classics and foundational theory

1. Kaplan, J. et al. (2020). "Scaling Laws for Neural Language Models." arXiv:2001.08361. ✅
2. Hoffmann, J. et al. (2022). "Training Compute-Optimal Large Language Models (Chinchilla)." arXiv:2203.15556. ✅
3. Maymounkov, P. & Mazieres, D. (2002). "Kademlia: A Peer-to-Peer Information System Based on the XOR Metric." IPTPS 2002. ✅
4. Douceur, J. R. (2002). "The Sybil Attack." IPTPS 2002. ✅
5. Adar, E. & Huberman, B. A. (2000). "Free Riding on Gnutella." First Monday, 5(10). ✅
6. Weyl, E. G. & Posner, E. A. (2018). Radical Markets. Princeton University Press. (Quadratic voting.) ✅

### P2P and decentralized systems

7. libp2p official docs. https://libp2p.io/ ✅ (accessed 2026-09-21)
8. IPFS official docs. https://docs.ipfs.tech/ ✅ (accessed 2026-09-21)
9. BOINC official site. https://boinc.berkeley.edu/ 🟡 (accessed 2026-09-21, scale figures project self-report)
10. Folding@home official site. https://foldingathome.org/ 🟡 (accessed 2026-09-21, peak compute project self-report, not independently reviewed)
11. Golem Network. https://www.golem.network/ 🟡 (accessed 2026-09-21, node count and throughput vendor self-report)
12. Akash Network. https://akash.network/ 🟡 (accessed 2026-09-21)
13. Render Network. https://rendernetwork.com/ 🟡 (accessed 2026-09-21)
14. io.net. https://io.net/ 🟡 (accessed 2026-09-21)
15. Bittensor. https://bittensor.com/ 🟡 (accessed 2026-09-21, subnet quality and real throughput not independently reviewed)
16. Filecoin. https://filecoin.io/ 🟡 (accessed 2026-09-21)

### Verifiable computation and privacy

17. Intel SGX official docs. https://www.intel.com/content/www/us/en/developer/tools/software-guard-extensions/overview.html ✅ (accessed 2026-09-21)
18. Intel TDX official docs. https://www.intel.com/content/www/us/en/products/docs/accelerator-engines/trust-domain-extensions.html ✅ (accessed 2026-09-21)
19. zkML-related research survey. [CITATION NEEDED] (no authoritative survey retrieved; concrete works like zkCNN, ZEN are academic papers 🟡)
20. Optimistic Rollup (Optimism/Arbitrum) official docs. ✅ (accessed 2026-09-21)

### Agent protocols and multi-agent systems

21. Model Context Protocol (MCP) official spec. https://modelcontextprotocol.io/ ✅ (accessed 2026-09-21)
22. Google A2A (Agent-to-Agent) official spec. https://google.github.io/A2A/ ✅ (accessed 2026-09-21)
23. MegaAgent: A Large-Scale Autonomous LLM-based Multi-Agent System Without Predefined SOPs. ACL 2025 Findings, DOI:10.18653/v1/2025.findings-acl.259. ✅ (590 agents, following v7.7.2 scope)
24. The Cost of Consensus in Multi-Agent Systems. arXiv:2605.00914. ✅ (sycophantic compliance 85.5%, oracle gap 32.3pp, multi-agent debate consuming 2.1-3.4× tokens with accuracy flat or lower; accessed 2026-09-21)
25. Malignant Epistemic Herding. arXiv:2605.06988. ✅ (agents converging through repeated belief fusion to shared but wrong beliefs, undetectable under standard coordination metrics; accessed 2026-09-21)
25. AgentCollabBench. arXiv:2605.08647. 🟡 (collaboration topology as an independent substrate dimension, following v7.7.2 scope)
26. Warp-Cortex. arXiv:2601.01298. 🟡 (single-author very thin preprint, only citing structural claims not figures, following v7.7.2 scope)

### RSI and self-improvement

27. RSI five-level framework ("The Last AI Built by Humans"). arXiv:2609.11873. ✅ (the five-level framework really exists, but "Theseus Labs" institutional attribution unconfirmed, changed to cite the arXiv original paper; accessed 2026-09-21)
28. Dream-RSI. arXiv:2609.14858. ✅ (Google/UMD/UVA/DeepMind, the idea "discovery tree is the world model"; accessed 2026-09-21)
29. Gödel Agent. arXiv:2410.04444. ✅ (self-referential framework, restricted-environment self-improvement, not end-to-end weight retraining; accessed 2026-09-21)
30. HyperAgents. arXiv:2603.19461. ✅ (Meta, task+meta agent editable programs, coding tasks +218%, cross-domain hypothesis fails; accessed 2026-09-21)
31. Introspection Threshold. arXiv:2607.04277. ✅ (systems lacking introspection self-modify converging to suboptimal fixed points; accessed 2026-09-21)

### Anti-Sybil and governance

32. World ID / Worldcoin official docs. https://worldcoin.org/ ✅ (accessed 2026-09-21, privacy disputes are public reports 🟡)
33. DAO governance research survey. [CITATION NEEDED] (voting participation and concentration figures are industry analyses 🟡, no authoritative academic survey retrieved)

### UDOS paper volumes

34. Fang Wenxin. UDOS v7.7.2 master volume 13 papers. 2026-09-21. (Paper 02 scaling law, Paper 04 RSI, Paper 05 coordination topology, Paper 06 consensus stopping, Paper 07 quality saturation, Paper 09 handoff contract, Paper 10 task market, Paper 13 evidence engineering.) verified, tracing release-v5.4.4/udos-engine/reports7/
35. Fang Wenxin. UDOS v7.5.0 master (P0/P0b/P0c/P1-P15). 2026-09.
36. Fang Wenxin. UDOS v7.6.0 master (P16-P34). 2026-09.
37. Fang Wenxin. UDOS v7.7.1 supplementary volume (P35-P37). 2026-09.

---

## Evidence discipline and reproduction notes

1. **UDOS's own figures**: the UDOS-system conclusions this paper cites (structural triplet S=(T,G,M), three-segment scaling-law curve, quality saturation law mse(k)=a+b/k, heterogeneity dividends, BFT-lite three-level finality, TransferBundle six fields, task-market settlement conservation) all trace back to the raw JSON in release-v5.4.4/udos-engine/reports7/, marked on three levels verified / cpu-proto / cpu-proxy / unverified. This paper adds no new UDOS simulation data, and all UDOS figures follow the v7.7.2 scope.

2. **External literature three states**: the external literature and industry systems this paper cites are marked on three states ✅ verified (with authoritative source URL, accessed 2026-09-21) / 🟡 paper-reported or company-disclosed, not independently reviewed / ⬛ no evidence found. Node counts, throughput, and unit costs of token projects like Bittensor, Golem, Akash, Render, io.net are all 🟡 (vendor self-report, not independently reviewed), not entering the verified column; but critical data like Akash CPU utilization 0.30%, io.net registered/online 49× gap, Bittensor revenue $3-15M and 175-400× revenue multiples have independent academic/on-chain sources ✅. BOINC/Folding@home scale figures are ✅ (official + academic sources). RSI-related five-level framework (arXiv:2609.11873), Dream-RSI (arXiv:2609.14858), Gödel Agent (arXiv:2410.04444), HyperAgents (arXiv:2603.19461) are all ✅ (really exist, but all are restricted self-improvement, none end-to-end rewriting weights); "Theseus Labs" institutional attribution ⬛ no evidence found, changed to cite the arXiv original paper.

3. **Design proposals**: this paper's eight-layer protocol stack, four protocol objects (Agent Manifest/Task Envelope/Receipt/Reputation), five-level verification hierarchy, dual-track economic model, contribution-metering formula, and six-stage cold-start roadmap are all "design proposals · to implement" + [RESULT NEEDED], not claiming implemented or verified. All JSON schemas are design drafts, and fields and values need iteration in testnets.

4. **Scaling-law exponents**: in the swarm-intelligence scaling-law formula this paper proposes, α,β,γ,δ,ε are all [RESULT NEEDED], needing calibration in the UDOS simulation environment or real testnets. This paper only presets 0<α<1 (sublinear), not presetting specific values.

5. **Forecasts**: this paper's 9 forecasts (F1-F9) are all scoring-time falsifiable forecasts, current status "pending." Forecasts are based on this paper's theoretical analysis and industry observation, not investment advice or technical promises.

6. **Compliance statement**: this paper does not encourage token speculation, promises no Token appreciation or returns, and is not investment advice. Credit is a non-transferable internal contribution-accounting unit with no monetary value. Token (if needed) must be compliantly issued, and fiat/stablecoin channels need KYC/AML compliance.

7. **Reproduction requirements**: to reproduce this paper's design proposals, one needs to: ① deploy libp2p nodes and Kademlia DHT; ② implement Agent Manifest content addressing and DHT registration; ③ implement Task Envelope publishing and matching; ④ implement at least L0 sampling and L1 redundant verification; ⑤ implement credit accounting and Reputation aggregation; ⑥ test task success rate, verification-cost ratio, and unit task cost at 100-1000 node scale. Reproduction results should be compared with this paper's key-metric targets.

8. **Limitations**: this paper is a theory-and-design paper and contains no experimental data from real P2P agent networks. All judgments about P2P network performance, cost, and scale are based on literature analysis and theoretical derivation and need verification in real testnets. This paper's critical analyses (like Bittensor's subnet quality problems, verification-cost death sentence) may change with technological progress and need continuous tracking.

---

*This paper is the UDOS v7.7.8 independent supplementary volume (P39), not merged into v7.7.2's 13-paper master volume. Author: Fang Wenxin. Date: 2026-09-21. Twin Earth · UDOS Reasoning Engine.*


---
