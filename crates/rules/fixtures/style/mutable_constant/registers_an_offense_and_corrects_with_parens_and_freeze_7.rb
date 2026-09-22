FOO = [1].freeze
BAR = [2].freeze
BAZ = [3].freeze
CONST = FOO + BAR + BAZ
        ^^^^^^^^^^^^^^^ Freeze mutable objects assigned to constants.
