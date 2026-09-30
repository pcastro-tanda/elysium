using Module.new {
  refine Hash do
    class << Hash
      private def _ruby2_keywords_hash(*args)
      end
    end
  end
}
