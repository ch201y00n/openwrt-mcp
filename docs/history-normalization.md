# Unpublished history normalization

On 2026-09-14 (Asia/Seoul), the user explicitly authorized rewriting unpublished
commit messages to Conventional Commits. The remote had no heads or tags at the
read-only preflight. Of 40 commits, six subjects changed; parent identity propagation
changed 33 commit IDs. Every source tree, author/committer identity and timestamp,
message body and topological order was preserved and compared. The index and all
tracked/untracked working changes were also verified unchanged. No push occurred.

The local recovery reference is `refs/backup/pre-conventional-20260914`, pointing
to original HEAD `15dae372fa85890037f05597d0d9b85e6e2bcd09`. The normalized HEAD at that point was
`f3059d3d308309153d793e6235a98f029d900adc`. This reference is not a remote branch, and should not be
published as part of ordinary development. Do not delete it or rewrite history
again without an explicit maintenance decision.

Earlier validation records retain the IDs observed when their tests actually ran.
The mapping below preserves traceability without relabeling historical artifacts
or changing their measured scope. Source-tree identity is exact for each pair;
subsequent implementation commits are not covered by that equality.

| Original commit | Conventional-message equivalent |
| --- | --- |
| `943dcf2854cf3f975ebe4a282a5fb9221a233560` | `cd2dd765c35e3936eee35b1edc33e5a49e0e9985` |
| `2bf6b4819f39ccd23253db788a9f0bdbb8027c94` | `63705b72b1025c6e67a62278d838fe0d81ed24ef` |
| `9918ce28eff502fac41a8192ff9ae16d3845804d` | `e782fde3dd620950452d10e89e04545cd8d2bcc8` |
| `eccd3e7010a615742d9baf48528898377d6180c2` | `604bf140ff836859b0d378bbf2181d0922929d76` |
| `8f6c09587cf34d627522b7da31c97df461e175c0` | `367cd703a4ecede800b5ed546f47ce517aaa0451` |
| `979029803271c81386118699b2669e372604ef15` | `ff465c648592bd2d3753ae7c0f7b17f713be1a7b` |
| `177f72016914decf73ef5f1326ec9d40ea9c7745` | `cef2a4f5a948e73823f520ba663d18ad48d98203` |
| `0521c7cf066958f7cea79817f6a4ca657c5db87e` | `2c75ea2a28e23ee183191319627ad56432b3b133` |
| `5d2878aeaff70fb68dff5c644e60c35ad886e8bf` | `777d0d0d969a7743a008b42a29973ba8e62a84ff` |
| `9c0117f582115fceff0d127374a7c55d7c4fcc49` | `86cf5508284a8e0899c168e630af8c64d8f356ab` |
| `0a43b73abf75779898c709b3cc57a4df74ee57da` | `5dd17abb41cb873947b72738f5ed498725bac0ce` |
| `acf65324adad7b509b714e8deba8ea199993f5e9` | `12852c24799678585001e5d497915fd21ca7c367` |
| `8c97589629c637c77a46d65ce5531a85523cbdd9` | `109df0e3186dfaa91fe20bace2771958d4070cf4` |
| `4040ce37ea4b09770ff268b038bcdcb810eeb42d` | `7f6c3270be79c9ac610492aeab9dd317fc48cc5e` |
| `e071f3e8ff8a8d39873f1a4f13fa86f1d936c38c` | `2c0244cbb8e1065e3ffc17505a8d934b23c7e8cf` |
| `7698fd1a79efd4b6f4b932f9a7da4992f56b5dd1` | `21674afde7f6a54d15290d89f60d2415fbde5b71` |
| `0b581eaceedfe81a8fd214b5df6875cc9633ccce` | `dbaae684d2d9a71a9fbc473f46c6c73fc1c8a69f` |
| `04893c97bd30759e924539055cb5bbdfbdb6f77a` | `651a95e6b754205725d4a91866fce7dbc54c3a6d` |
| `6c45623914ef327d6e8e007f8644e22905eda620` | `285a76b58a602927d8e5f53b6c4634d368c7200b` |
| `09e4816b2c44e3f7b203a7b02b76f7dd5dac8348` | `669555254485248cba247e557723ce3d6414afbf` |
| `fe677cb2674b900972b1a1b219cc4bb6e8d9013c` | `3292b75f4e1099db68ac023c4e087056e59b7513` |
| `82853f67d894161837e05e756bc7b99ad4c9f750` | `69cd92909fdbbca5c50cc654bcb0850b7fe90465` |
| `2233f3e83a63313e98bcf9413c3efa78ea6ee042` | `3510425633b6cee7db140f072e90d376cbed94a9` |
| `a8c24bf0469b672754012577a3c51e68f30c0754` | `f8f8de4e25a09985830c902d628f2a2777b4c9c0` |
| `8adc3278df83289bff7529fb483c90edbbe1db49` | `5273e87d338b390396cd61f358bd10f953a422e1` |
| `a3a34a28b69a3f7855f700fab14ab3c963d07a3a` | `8e17940d0f2ff8ec1b52c087c2b9f465c8e76e96` |
| `6c3e2564459ab78fce7ff418c07bebf6062d2631` | `0f1f59642bd629ca3c1a9d5e42175623cf19b848` |
| `75fafa862c97dea7f65cbba93e94ee627257a6b1` | `c62b10b596469184c2aba968f67c1ff10656811d` |
| `e72e685361e86f90516d3985c06ef749ce124bb2` | `f7175585915c85c089ff440c532df874ec5ae36d` |
| `a04e056bef1bcabae7b9d17b8f7fe6fc6a048224` | `3fd563973e14b46e6b949e6cf8c84a96d3c21e22` |
| `47cf2e40e8f82bd28eabe9ffa16018369f0c1443` | `a31acdb792aab5d183ef0963f35160068468807f` |
| `50048e62f495551ea65c9ec8b1987183da0c88b4` | `2b7a1805a3f5967862df0347a51ee16b3dd5be2a` |
| `15dae372fa85890037f05597d0d9b85e6e2bcd09` | `f3059d3d308309153d793e6235a98f029d900adc` |
