// Renderer fixture: every circuit feature once.
#import "/typst/lecfig/lib.typ": render
#set page(width: auto, height: auto, margin: 1cm)

#render(json("circuit.json"))
