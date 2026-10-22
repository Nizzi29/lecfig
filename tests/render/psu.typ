// Renderer fixture: labeled blocks (computer power supply from the lecture).
#import "/typst/lecfig/lib.typ": render
#set page(width: auto, height: auto, margin: 1cm)

#render(json("psu.json"))
