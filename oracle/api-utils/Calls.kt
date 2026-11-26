package oracle

import core.api.utils.WeightedTable

// WeightedTable's members as Kotlin content calls them (Java would resolve `remove(Object)` and `remove(int)`
// through ArrayList's bridges instead).
fun tableAdd(table: WeightedTable<Any>, element: Any?, weight: Double): Boolean = table.add(element, weight)
fun tableAddRaw(table: WeightedTable<Any>, element: Any?, weight: Double): Boolean = table.add(Pair(element, weight))
fun tableRemove(table: WeightedTable<Any>, element: Any?): Boolean = table.remove(element)
fun tableRemoveAt(table: WeightedTable<Any>, index: Int): Pair<Any?, Double> = table.removeAt(index)
fun tableRoll(table: WeightedTable<Any>): Any? = table.roll()
fun tableClear(table: WeightedTable<Any>) = table.clear()
fun tableCreate(elements: List<Pair<Any?, Double>>): WeightedTable<Any> = WeightedTable.create(*elements.toTypedArray())
fun tableEntries(table: WeightedTable<Any>): List<Pair<Any?, Double>> = table.toList()
