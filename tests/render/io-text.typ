// Renderer fixture: schema 2, input/output labels as text with $math$.
#import "/typst/lecfig/lib.typ": render
#set page(width: auto, height: auto, margin: 1cm)

#render(json("io-text.json"))
