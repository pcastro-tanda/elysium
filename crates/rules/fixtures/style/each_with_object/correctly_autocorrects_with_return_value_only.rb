[1, 2, 3].inject({}) do |h, i|
          ^^^^^^ Use `each_with_object` instead of `inject`.
  h
end
