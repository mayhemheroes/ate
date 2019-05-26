package com.tokera.ate.annotations;

import java.lang.annotation.*;

/**
 * Indicates that this data object has implicit authority from a specific DNS address
 * @author John Sharratt (johnathan.sharratt@gmail.com)
 */
@Target(value = {ElementType.TYPE})
@Retention(value = RetentionPolicy.RUNTIME)
@Documented
public @interface ImplicitAuthority {

    String value();
}
