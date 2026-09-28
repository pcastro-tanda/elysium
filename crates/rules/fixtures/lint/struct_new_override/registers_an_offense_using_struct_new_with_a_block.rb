Struct.new(:members) do
           ^^^^^^^^ `:members` member overrides `Struct#members` and it may be unexpected.
  def members?
    !members.empty?
  end
end
