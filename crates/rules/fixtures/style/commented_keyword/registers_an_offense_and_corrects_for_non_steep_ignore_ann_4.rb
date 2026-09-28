class X # steep
        ^^^^^^^ Do not place comments on the same line as the `class` keyword.
end

class X #steep:ignore
        ^^^^^^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end

class X # steep:ignoreUnknownConstant
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
