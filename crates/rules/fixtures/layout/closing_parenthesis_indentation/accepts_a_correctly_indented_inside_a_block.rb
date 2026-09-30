block_adds_extra_indentation do
  some_method(a,
    x: 1,
    y: 2
  )
  b =
    some_method(a,
               )
end
