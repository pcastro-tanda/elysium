module SomeModule
  extend ActiveSupport::Concern
  class_methods do
    def some_public_class_method
    end
    private
    def some_private_class_method
    end
  end
  def some_public_instance_method
  end
  def some_private_instance_method
  end
end
