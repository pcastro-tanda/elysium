def citations = a_method_call[1..].filter_map do |argument|
  some_other_method(argument)
end
