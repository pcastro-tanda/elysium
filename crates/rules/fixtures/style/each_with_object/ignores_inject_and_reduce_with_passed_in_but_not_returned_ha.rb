[].inject({}) do |a, e|
  a + e
end

[].reduce({}) do |a, e|
  my_method e, a
end
