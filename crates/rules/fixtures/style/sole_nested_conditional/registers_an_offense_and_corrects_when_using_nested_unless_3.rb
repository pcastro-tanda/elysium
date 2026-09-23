class A
  def foo
    if h[:a]
      h[:b] = true unless h.has_key?(:b)
                   ^^^^^^ Consider merging nested conditions into outer `if` conditions.
    end
  end
end
