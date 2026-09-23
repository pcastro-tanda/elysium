def foo(**)
  bar ab: 1,
      c: 2, **
      ^^^^ Align the keys and values of a hash literal if they span more than one line.
end
