# KataPass
Wrap GTP engines so that the AI passes when it thinks it can afford to.

Usage:
```
katapass.exe <path_to_katapass_config>
```

KataPass config file must include 3 fields under the ```[katapass]``` section:
1. engine: The path to the GTP engine you wish to use.
2. args: The arguments to the engine.
3. intercept: The GTP genmove or equivalent command you wish to intercept.  This command must satisfy the following criteria:
    * It must take a 'B' or 'W' as its second argument.
    * It must report a numeric winrate between 0.0 and 1.0 after the word 'winrate' in its output.
    
Paths in the config file may not include spaces, sorry.
