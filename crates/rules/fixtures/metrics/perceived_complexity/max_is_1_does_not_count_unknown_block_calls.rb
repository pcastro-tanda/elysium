def method_name
  bar.baz(:qux) do |x|
    raise x
  end
end
