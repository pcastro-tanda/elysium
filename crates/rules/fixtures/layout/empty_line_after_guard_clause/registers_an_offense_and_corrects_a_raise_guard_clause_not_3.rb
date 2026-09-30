def foo
  raise SerializationError.new("Unsupported argument type: #{argument.class.name}") unless serializer
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  serializer.serialize(argument)
end
