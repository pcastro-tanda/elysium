module X # steep
         ^^^^^^^ Do not place comments on the same line as the `module` keyword.
end

module X #steep:ignore
         ^^^^^^^^^^^^^ Do not place comments on the same line as the `module` keyword.
end

module X # steep:ignoreUnknownConstant
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not place comments on the same line as the `module` keyword.
end
