class MyClass
  def public_method
  end

  private

  def first_private_method
  end

  def second_private_method
  end

  protected

  def first_protected_method
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ `protected_methods` is supposed to appear before `private_methods`.
  end

  def second_protected_method
  end
end
