# Byte-stream transcript fixtures for labs::prompt_detect / labs::evaluator tests
#
# To regenerate (run from this directory):
#
# kubectl-get-pods.bytes — OSC 133 canonical (PromptStart -> CommandStart -> OutputStart -> CommandEnd exit=0)
printf '\033]133;A\007$ \033]133;B\007kubectl get pods\n\033]133;C\007NAME READY STATUS\nweb 1/1 Running\n\033]133;D;0\007$ \033]133;B\007' > kubectl-get-pods.bytes
#
# exit-zero.bytes — minimal OSC 133 sequence ending with D ;0
printf '\033]133;A\007$ \033]133;B\007ls\n\033]133;C\007foo bar\n\033]133;D;0\007' > exit-zero.bytes
#
# exit-nonzero.bytes — minimal OSC 133 sequence ending with D ;127 (command-not-found)
printf '\033]133;A\007$ \033]133;B\007zzz\n\033]133;C\007zsh: command not found: zzz\n\033]133;D;127\007' > exit-nonzero.bytes
#
# no-osc-133.bytes — plain bash transcript without OSC 133 (heuristic-fallback target)
printf '$ ls\nfoo\nbar\n$ ' > no-osc-133.bytes
