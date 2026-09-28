class Foo
  def self.define(mod)
    mod.module_eval do
      def y
      end
    end
  end
end
