class Foo
  def citations
    a_method_call[1..].filter_map { |argument| some_other_method(argument) }
  end
end
