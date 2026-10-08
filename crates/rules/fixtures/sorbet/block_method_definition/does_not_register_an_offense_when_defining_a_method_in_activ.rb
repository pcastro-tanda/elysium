module SomeConcern
  extend ActiveSupport::Concern

  class_methods do
    def good_method(args)
      args.present?
    end
  end
end
