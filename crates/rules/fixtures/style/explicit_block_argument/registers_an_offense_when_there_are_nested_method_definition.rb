def foo
  def bar.baz
    qux { |quux| yield quux }
    ^^^^^^^^^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
  end
end
