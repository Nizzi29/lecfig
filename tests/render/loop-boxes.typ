// Renderer fixture: generic blocks with text labels (control loop from the lecture).
#import "/typst/lecfig/lib.typ": render
#set page(width: auto, height: auto, margin: 1cm)

#render(json("loop-boxes.json"))
