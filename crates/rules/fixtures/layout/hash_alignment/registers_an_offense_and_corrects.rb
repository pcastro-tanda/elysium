def foo(**)
  bar a: 1,
        **
        ^^ Align keyword splats with the rest of the hash if it spans more than one line.
end
