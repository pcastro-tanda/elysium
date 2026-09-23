class A
  def foo
    if (h[:a]) && !h.has_key?(:b)
      h[:b] = true
    end
  end
end
