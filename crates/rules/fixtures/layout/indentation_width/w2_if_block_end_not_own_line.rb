a_function {
  # a comment
  result = AObject.find_by_attr(attr) if attr
  result || AObject.make(
      :attr => attr,
      :attr2 => Other.get_value(),
      :attr3 => Another.get_value()) }
