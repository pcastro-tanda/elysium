# shareable_constant_value: literal
X = [1, 2, 3]
# shareable_constant_value: none
Y = [4, 5, 6]
    ^^^^^^^^^ Freeze mutable objects assigned to constants.
# shareable_constant_value: experimental_everything
Z = [7, 8, 9]
