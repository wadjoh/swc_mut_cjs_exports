export { };
Object.defineProperty(exports, "mod", {
    enumerable: true,
    get () {
        return mod;
    },
    set (v) {
        mod = v;
    },
    configurable: true
});
Object.defineProperty(exports, "foo", {
    enumerable: true,
    get () {
        return foo;
    },
    set (v) {
        foo = v;
    },
    configurable: true
});
Object.defineProperty(exports, "bar", {
    enumerable: true,
    get () {
        return bar;
    },
    set (v) {
        bar = v;
    },
    configurable: true
});
Object.defineProperty(exports, "baz", {
    enumerable: true,
    get () {
        return baz;
    },
    set (v) {
        baz = v;
    },
    configurable: true
});
Object.keys(mod1).forEach(function(key) {
    if (key === "default" || key === "__esModule") return;
    if (key in exports && exports[key] === mod1[key]) return;
    exports[key] = mod1[key];
});
import * as mod from "./someModule";
import * as mod1 from "./someModule";
import { foo as foo, bar as bar, baz as baz } from "./someModule";
