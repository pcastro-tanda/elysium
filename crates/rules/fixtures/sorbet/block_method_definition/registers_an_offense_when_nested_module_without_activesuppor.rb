module OuterModule
  extend ActiveSupport::Concern

  module InnerModule
    class_methods do
      def bad_method(args)
      ^^^^^^^^^^^^^^^^^^^^ Sorbet/BlockMethodDefinition: Do not define methods in blocks (use `define_method` as a workaround).
        args.present?
      end
    end
  end
end
