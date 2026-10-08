Data.define(:members) do
            ^^^^^^^^ `:members` member overrides `Data#members` and it may be unexpected.
  def members?
    !members.empty?
  end
end
