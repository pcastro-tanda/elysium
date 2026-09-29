[].inject({}) { |a, e| a }
   ^^^^^^ Use `each_with_object` instead of `inject`.

[].reduce({}) do |a, e|
   ^^^^^^ Use `each_with_object` instead of `reduce`.
  a[e] = 1
  a[e] = 1
  a
end
