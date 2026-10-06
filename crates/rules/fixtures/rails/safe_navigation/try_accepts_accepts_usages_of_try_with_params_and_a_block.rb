[1, 2].try(:each_with_object, []) do |e, acc|
  acc << e.some_method
end