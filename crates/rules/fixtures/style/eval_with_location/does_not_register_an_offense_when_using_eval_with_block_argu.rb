def self.included(base)
  base.class_eval do
    include OtherModule
  end
end
