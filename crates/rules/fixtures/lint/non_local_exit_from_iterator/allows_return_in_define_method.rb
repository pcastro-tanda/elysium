[:method_one, :method_two].each do |method_name|
  define_method(method_name) do
    return if predicate?
  end
end
