class Foo
  def require(p)  # rubocop:disable Naming/MethodParameterName
                ^ Unnecessary spacing detected.
  end

  def load(p)  # rubocop:disable Naming/MethodParameterName
             ^ Unnecessary spacing detected.
  end

  def join(*ps)  # rubocop:disable Naming/MethodParameterName
               ^ Unnecessary spacing detected.
  end

  def exist?(*ps)  # rubocop:disable Naming/MethodParameterName
                 ^ Unnecessary spacing detected.
  end
end
