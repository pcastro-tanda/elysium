class Foo
  def self.define(mod)
    mod.module_exec do
      def y
      end
    end
  end
end
