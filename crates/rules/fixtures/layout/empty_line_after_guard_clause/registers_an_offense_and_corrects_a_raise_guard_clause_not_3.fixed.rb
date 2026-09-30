def foo
  raise SerializationError.new("Unsupported argument type: #{argument.class.name}") unless serializer

  serializer.serialize(argument)
end
