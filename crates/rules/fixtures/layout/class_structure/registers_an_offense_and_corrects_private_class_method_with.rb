class A
  private

  def do_something
  end

  private_class_method def self.do_internal_work
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `private_class_methods` is supposed to appear before `private_methods`.
  end
end
