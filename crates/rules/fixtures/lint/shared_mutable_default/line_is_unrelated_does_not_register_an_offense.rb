[]
{}
Array.new
Hash.new
Hash.new { |h, k| h[k] = [] }
Hash.new { |h, k| h[k] = {} }
Hash.new { 0 }
Hash.new { Array.new }
Hash.new { Hash.new }
Hash.new { {} }
Hash.new { [] }
Hash.new(0)
Hash.new(false)
Hash.new(true)
Hash.new(nil)
Hash.new(BigDecimal(0))
Hash.new(BigDecimal(0.0))
Hash.new(0.0)
Hash.new(0.0.to_d)
