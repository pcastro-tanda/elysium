m = Module.new
n = Module.new
module m::n::M
^^^^^^^^^^^^^^ Missing top-level documentation comment for `module m::n::M`.
  def method
  end
end
