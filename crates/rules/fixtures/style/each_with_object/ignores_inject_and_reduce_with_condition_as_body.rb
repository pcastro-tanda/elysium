[].inject({}) do |a, e|
  a = e if e
end

[].inject({}) do |a, e|
  if e
    a = e
  end
end

[].reduce({}) do |a, e|
  a = e ? e : 2
end
